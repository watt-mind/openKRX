//! Bounded entry decoding with streaming limit enforcement and CRC-32 checking.
//!
//! Decoding never buffers a whole entry unless a caller explicitly asks for its
//! bytes: the deflate path keeps one [`Limits::OUTPUT_BUFFER_BYTES`] output
//! buffer and discards each chunk after accounting for it, while a stored entry
//! is accounted for in place and allocates nothing. Limits are therefore
//! enforced against actual decoded bytes, and no limit can be overshot by more
//! than one buffer before decoding aborts.
//!
//! CRC-32 checking detects corruption. It is not an authenticity check.

use miniz_oxide::inflate::stream::{InflateState, inflate};
use miniz_oxide::{DataFormat, MZFlush, MZStatus};

use crate::error::{ArchiveError, LimitKind, MalformedKind};
use crate::limits::Limits;

/// Stored (uncompressed) compression method.
pub(crate) const METHOD_STORED: u16 = 0;
/// Deflate compression method.
pub(crate) const METHOD_DEFLATE: u16 = 8;

/// What a caller wants back from a decode pass.
pub(crate) enum Sink<'a> {
    /// Count and discard decoded bytes.
    Count,
    /// Additionally retain decoded bytes, bounded by the per-entry limit.
    Collect(&'a mut Vec<u8>),
}

/// Everything a decode pass needs about one entry and the budget it shares.
pub(crate) struct Decode<'a> {
    /// Exactly the entry's compressed bytes.
    pub(crate) data: &'a [u8],
    /// Compression method, already validated as stored or deflate.
    pub(crate) method: u16,
    /// Uncompressed size declared by the central directory.
    pub(crate) declared_size: u64,
    /// CRC-32 declared by the central directory.
    pub(crate) declared_crc: u32,
    /// Central-directory index, used for diagnostics only.
    pub(crate) entry: u32,
    /// Decoded bytes already accounted for by earlier entries.
    pub(crate) already_decoded: u64,
}

/// Decode one entry, enforcing limits per output chunk.
///
/// Returns the counted decoded size, which is compared against the declared
/// size by the caller as well as chunk-by-chunk here.
pub(crate) fn decode(
    request: &Decode<'_>,
    limits: &Limits,
    mut sink: Sink<'_>,
) -> Result<u64, ArchiveError> {
    let mut accounting = Accounting {
        request,
        limits,
        decoded: 0,
        crc: Crc32::new(),
    };
    match request.method {
        METHOD_STORED => {
            for chunk in request.data.chunks(Limits::OUTPUT_BUFFER_BYTES) {
                accounting.accept(chunk, &mut sink)?;
            }
        }
        _ => {
            let mut buffer = vec![0_u8; Limits::OUTPUT_BUFFER_BYTES];
            inflate_all(request, &mut buffer, &mut accounting, &mut sink)?;
        }
    }
    accounting.finish()
}

/// Run the streaming inflate loop over the entry's compressed bytes.
fn inflate_all(
    request: &Decode<'_>,
    buffer: &mut [u8],
    accounting: &mut Accounting<'_, '_>,
    sink: &mut Sink<'_>,
) -> Result<(), ArchiveError> {
    let malformed = || ArchiveError::Malformed {
        kind: MalformedKind::DeflateStream,
        entry: Some(request.entry),
    };
    let mut state = InflateState::new_boxed(DataFormat::Raw);
    let mut consumed = 0_usize;
    loop {
        let result = inflate(&mut state, &request.data[consumed..], buffer, MZFlush::None);
        consumed = consumed
            .checked_add(result.bytes_consumed)
            .ok_or_else(malformed)?;
        if result.bytes_written > 0 {
            accounting.accept(&buffer[..result.bytes_written], sink)?;
        }
        match result.status {
            Ok(MZStatus::StreamEnd) => break,
            Ok(MZStatus::Ok) if result.bytes_consumed > 0 || result.bytes_written > 0 => {}
            _ => return Err(malformed()),
        }
    }
    if consumed == request.data.len() {
        Ok(())
    } else {
        Err(malformed())
    }
}

/// Per-chunk limit accounting, CRC state and the optional collection sink.
struct Accounting<'a, 'b> {
    request: &'a Decode<'b>,
    limits: &'a Limits,
    decoded: u64,
    crc: Crc32,
}

impl Accounting<'_, '_> {
    /// Account for one decoded chunk, aborting as soon as a bound is crossed.
    fn accept(&mut self, chunk: &[u8], sink: &mut Sink<'_>) -> Result<(), ArchiveError> {
        let entry = Some(self.request.entry);
        let over = |limit, limit_value, observed| ArchiveError::OverLimit {
            limit,
            limit_value,
            observed: Some(observed),
            entry,
        };
        self.decoded =
            self.decoded
                .checked_add(chunk.len() as u64)
                .ok_or(ArchiveError::OverLimit {
                    limit: LimitKind::EntryDecodedBytes,
                    limit_value: self.limits.max_entry_decoded_bytes,
                    observed: None,
                    entry,
                })?;
        if self.decoded > self.request.declared_size {
            return Err(ArchiveError::Malformed {
                kind: MalformedKind::DeclaredSizeMismatch,
                entry,
            });
        }
        if self.decoded > self.limits.max_entry_decoded_bytes {
            return Err(over(
                LimitKind::EntryDecodedBytes,
                self.limits.max_entry_decoded_bytes,
                self.decoded,
            ));
        }
        let total = self.request.already_decoded.saturating_add(self.decoded);
        if total > self.limits.max_total_decoded_bytes {
            return Err(over(
                LimitKind::TotalDecodedBytes,
                self.limits.max_total_decoded_bytes,
                total,
            ));
        }
        let compressed = (self.request.data.len() as u64).max(1);
        let ratio = self.decoded / compressed;
        if self.decoded > Limits::RATIO_GRACE_BYTES && ratio > self.limits.max_compression_ratio {
            return Err(over(
                LimitKind::CompressionRatio,
                self.limits.max_compression_ratio,
                ratio,
            ));
        }
        self.crc.update(chunk);
        if let Sink::Collect(buffer) = sink {
            buffer.extend_from_slice(chunk);
        }
        Ok(())
    }

    /// Verify the declared size and CRC-32 once the stream has ended.
    fn finish(self) -> Result<u64, ArchiveError> {
        let entry = Some(self.request.entry);
        if self.decoded != self.request.declared_size {
            return Err(ArchiveError::Malformed {
                kind: MalformedKind::DeclaredSizeMismatch,
                entry,
            });
        }
        if self.crc.value() != self.request.declared_crc {
            return Err(ArchiveError::Malformed {
                kind: MalformedKind::CrcMismatch,
                entry,
            });
        }
        Ok(self.decoded)
    }
}

/// Table-driven CRC-32 (IEEE 802.3 polynomial), as used by the ZIP format.
pub(crate) struct Crc32 {
    state: u32,
}

const CRC_TABLE: [u32; 256] = {
    let mut table = [0_u32; 256];
    let mut index = 0;
    while index < 256 {
        let mut value = index as u32;
        let mut bit = 0;
        while bit < 8 {
            value = if value & 1 == 1 {
                0xedb8_8320 ^ (value >> 1)
            } else {
                value >> 1
            };
            bit += 1;
        }
        table[index] = value;
        index += 1;
    }
    table
};

impl Crc32 {
    /// Start a fresh CRC-32 computation.
    pub(crate) const fn new() -> Self {
        Self { state: !0 }
    }

    /// Fold one chunk into the running value.
    pub(crate) fn update(&mut self, chunk: &[u8]) {
        for byte in chunk {
            let index = usize::from((self.state as u8) ^ *byte);
            self.state = CRC_TABLE[index] ^ (self.state >> 8);
        }
    }

    /// Finalise and return the CRC-32 value.
    pub(crate) const fn value(&self) -> u32 {
        !self.state
    }
}

#[cfg(test)]
mod tests {
    use super::Crc32;

    #[test]
    fn crc32_matches_published_check_value() {
        let mut crc = Crc32::new();
        crc.update(b"123456789");
        assert_eq!(crc.value(), 0xcbf4_3926);
    }

    #[test]
    fn crc32_of_empty_input_is_zero() {
        assert_eq!(Crc32::new().value(), 0);
    }

    #[test]
    fn crc32_is_order_dependent_across_chunks() {
        let mut split = Crc32::new();
        split.update(b"1234");
        split.update(b"56789");
        let mut whole = Crc32::new();
        whole.update(b"123456789");
        assert_eq!(split.value(), whole.value());
    }
}
