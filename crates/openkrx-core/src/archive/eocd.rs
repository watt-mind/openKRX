//! End-of-central-directory location and parsing.
//!
//! A candidate is a `PK\x05\x06` signature whose 22-byte record fits and whose
//! declared comment ends exactly at the end of the image. Exactly one candidate
//! must exist: two candidates are an ambiguity and are rejected rather than
//! resolved by "take the last", and a signature whose comment stops short means
//! the image carries trailing bytes.

use crate::error::{AmbiguityKind, ArchiveError, LimitKind, MalformedKind, UnsupportedKind};
use crate::limits::Limits;

/// End-of-central-directory record signature.
pub(crate) const EOCD_SIGNATURE: [u8; 4] = [b'P', b'K', 5, 6];
/// ZIP64 end-of-central-directory locator signature.
const ZIP64_LOCATOR_SIGNATURE: [u8; 4] = [b'P', b'K', 6, 7];
/// Fixed size of an end-of-central-directory record, comment excluded.
pub(crate) const EOCD_FIXED_BYTES: usize = 22;

/// The parsed end-of-central-directory record.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Eocd {
    /// Offset of the record itself.
    pub(crate) offset: usize,
    /// Number of central-directory records the record declares.
    pub(crate) entries: u32,
    /// Declared central-directory offset.
    pub(crate) directory_offset: u64,
}

/// Locate and parse the single end-of-central-directory record.
pub(crate) fn find(bytes: &[u8], limits: &Limits) -> Result<Eocd, ArchiveError> {
    let mut terminal = None;
    let mut signatures = 0_usize;
    for start in 0..bytes.len().saturating_sub(3) {
        if bytes[start..start + 4] != EOCD_SIGNATURE {
            continue;
        }
        signatures += 1;
        let Some(record) = bytes[start..].first_chunk::<EOCD_FIXED_BYTES>() else {
            continue;
        };
        let comment = usize::from(u16::from_le_bytes([record[20], record[21]]));
        if start + EOCD_FIXED_BYTES + comment != bytes.len() {
            continue;
        }
        if terminal.is_some() {
            return Err(ArchiveError::Ambiguous {
                kind: AmbiguityKind::Eocd,
                entry: None,
            });
        }
        terminal = Some((start, record));
    }
    match terminal {
        Some((start, record)) => parse(bytes, start, record, limits),
        None if signatures > 0 => Err(ArchiveError::Malformed {
            kind: MalformedKind::TrailingBytes,
            entry: None,
        }),
        None => Err(ArchiveError::Malformed {
            kind: MalformedKind::EocdMissing,
            entry: None,
        }),
    }
}

/// Parse and validate the record `find` located at `start`.
///
/// The record arrives as its 22 fixed bytes, which `find` has already proven
/// present, so every field is read at a constant offset and no read here can
/// run off the end of the image.
fn parse(
    bytes: &[u8],
    start: usize,
    record: &[u8; EOCD_FIXED_BYTES],
    limits: &Limits,
) -> Result<Eocd, ArchiveError> {
    if start >= 4 && bytes[start - 4..start] == ZIP64_LOCATOR_SIGNATURE {
        return Err(unsupported(UnsupportedKind::Zip64));
    }
    let disk = u16::from_le_bytes([record[4], record[5]]);
    let directory_disk = u16::from_le_bytes([record[6], record[7]]);
    let entries_here = u16::from_le_bytes([record[8], record[9]]);
    let entries_total = u16::from_le_bytes([record[10], record[11]]);
    let directory_bytes = u32::from_le_bytes([record[12], record[13], record[14], record[15]]);
    let directory_offset = u32::from_le_bytes([record[16], record[17], record[18], record[19]]);
    let comment = u16::from_le_bytes([record[20], record[21]]);
    if disk != 0 || directory_disk != 0 || entries_here != entries_total {
        return Err(unsupported(UnsupportedKind::MultiDisk));
    }
    if entries_total == u16::MAX
        || directory_bytes == u32::MAX
        || directory_offset == u32::MAX
        || disk == u16::MAX
    {
        return Err(unsupported(UnsupportedKind::Zip64));
    }
    if u64::from(comment) > u64::from(limits.max_comment_bytes) {
        return Err(ArchiveError::OverLimit {
            limit: LimitKind::CommentBytes,
            limit_value: u64::from(limits.max_comment_bytes),
            observed: Some(u64::from(comment)),
            entry: None,
        });
    }
    if u64::from(entries_total) > u64::from(limits.max_entries) {
        return Err(ArchiveError::OverLimit {
            limit: LimitKind::Entries,
            limit_value: u64::from(limits.max_entries),
            observed: Some(u64::from(entries_total)),
            entry: None,
        });
    }
    let directory_end = u64::from(directory_offset)
        .checked_add(u64::from(directory_bytes))
        .ok_or(ArchiveError::Malformed {
            kind: MalformedKind::CentralDirectorySize,
            entry: None,
        })?;
    if directory_end != start as u64 {
        return Err(ArchiveError::Malformed {
            kind: MalformedKind::CentralDirectoryPlacement,
            entry: None,
        });
    }
    Ok(Eocd {
        offset: start,
        entries: u32::from(entries_total),
        directory_offset: u64::from(directory_offset),
    })
}

/// Build an archive-scoped unsupported-feature error.
const fn unsupported(kind: UnsupportedKind) -> ArchiveError {
    ArchiveError::Unsupported {
        kind,
        value: None,
        entry: None,
    }
}
