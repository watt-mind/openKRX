//! Bounded, profile-agnostic ZIP archive inventory.
//!
//! [`inventory`] reads a caller-supplied byte image and reports what the archive
//! actually contains. It performs no filesystem, clock, process or network
//! access, and it never labels an archive a conforming KRX package: rules A19
//! to A22 in `docs/profile.md` are unresolved, so the inventory reports
//! observations only and leaves every profile judgement to a later layer.
//!
//! The reader is deliberately strict. It rejects ambiguity instead of choosing a
//! convenient reading, checks local headers against the central directory,
//! requires the image to be exactly covered by the structures it declares, and
//! enforces every limit in [`Limits`] against decoded bytes.
//!
//! ```
//! use openkrx_core::{Limits, archive};
//!
//! # fn demo(image: &[u8]) -> Result<(), openkrx_core::ArchiveError> {
//! let inventory = archive::inventory(image, &Limits::DEFAULT)?;
//! for entry in inventory.entries() {
//!     let _ = (entry.name_bytes(), entry.decoded_size());
//! }
//! # Ok(())
//! # }
//! ```

mod central;
mod eocd;
mod inflate;
mod kind;
mod local;
pub(crate) mod names;
mod raw;

use crate::error::{ArchiveError, LimitKind, MalformedKind, Structure};
use crate::limits::Limits;

pub use kind::EntryKind;

pub(crate) use inflate::Crc32;

use central::CentralRecord;
use inflate::{Decode, Sink};
use local::EntryLayout;
use raw::offset;

/// One archive entry, described exactly as the archive declares it.
///
/// Nothing here is a conformance statement: the type reports observed metadata
/// and the decoded size actually produced while reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchiveEntry<'a> {
    name: &'a [u8],
    name_text: Option<&'a str>,
    utf8_flag: bool,
    flags: u16,
    method: u16,
    compressed_size: u64,
    uncompressed_size: u64,
    crc32: u32,
    local_header_offset: u64,
    decoded_size: u64,
    version_made_by: u16,
    external_attributes: u32,
}

impl<'a> ArchiveEntry<'a> {
    /// Raw name bytes, exactly as stored, with no encoding interpretation.
    #[must_use]
    pub const fn name_bytes(&self) -> &'a [u8] {
        self.name
    }

    /// The name as text when the raw bytes are valid UTF-8.
    ///
    /// This is independent of [`ArchiveEntry::utf8_flag`]: an archive may set the
    /// flag without storing UTF-8, or store UTF-8 without setting it. Entry-name
    /// encoding is unresolved rule A21, so both facts are reported separately.
    #[must_use]
    pub const fn name_text(&self) -> Option<&'a str> {
        self.name_text
    }

    /// Whether general-purpose bit 11 declares the name to be UTF-8.
    #[must_use]
    pub const fn utf8_flag(&self) -> bool {
        self.utf8_flag
    }

    /// The entry's general-purpose bit flags.
    #[must_use]
    pub const fn flags(&self) -> u16 {
        self.flags
    }

    /// Compression method: 0 for stored, 8 for deflate.
    #[must_use]
    pub const fn method(&self) -> u16 {
        self.method
    }

    /// Compressed size declared by the central directory.
    #[must_use]
    pub const fn compressed_size(&self) -> u64 {
        self.compressed_size
    }

    /// Uncompressed size declared by the central directory.
    #[must_use]
    pub const fn uncompressed_size(&self) -> u64 {
        self.uncompressed_size
    }

    /// CRC-32 declared by the central directory.
    #[must_use]
    pub const fn crc32(&self) -> u32 {
        self.crc32
    }

    /// Offset of this entry's local file header.
    #[must_use]
    pub const fn local_header_offset(&self) -> u64 {
        self.local_header_offset
    }

    /// The central directory's `version made by` field.
    ///
    /// Its high byte names the host system that wrote the record, which decides
    /// how [`ArchiveEntry::external_attributes`] is to be read.
    #[must_use]
    pub const fn version_made_by(&self) -> u16 {
        self.version_made_by
    }

    /// The central directory's `external file attributes` field.
    ///
    /// Its meaning depends on the host system: a Unix host stores `st_mode` in
    /// the high 16 bits, and an MS-DOS or NTFS host stores FAT attribute bits.
    #[must_use]
    pub const fn external_attributes(&self) -> u32 {
        self.external_attributes
    }

    /// What the entry declares itself to be.
    ///
    /// This is a reading of the two fields above together with the name, never
    /// a filesystem fact. [`crate::extract::plan`] refuses a [`EntryKind::Symlink`]
    /// and a [`EntryKind::Special`] entry rather than planning output for it.
    #[must_use]
    pub fn kind(&self) -> EntryKind {
        kind::classify(self.version_made_by, self.external_attributes, self.name)
    }

    /// Bytes actually produced while decoding this entry.
    ///
    /// The inventory only succeeds when this equals
    /// [`ArchiveEntry::uncompressed_size`]; the counted value is reported so a
    /// consumer never has to trust the declaration.
    #[must_use]
    pub const fn decoded_size(&self) -> u64 {
        self.decoded_size
    }
}

/// What an archive image was observed to contain.
///
/// The inventory borrows the image so a single entry can be decoded again on
/// demand; no decoded payload is retained.
#[derive(Debug, Clone)]
pub struct ArchiveInventory<'a> {
    image: &'a [u8],
    entries: Vec<ArchiveEntry<'a>>,
    data_ranges: Vec<(usize, usize)>,
    total_decoded_bytes: u64,
    limits: Limits,
}

impl<'a> ArchiveInventory<'a> {
    /// Entries in central-directory order.
    #[must_use]
    pub fn entries(&self) -> &[ArchiveEntry<'a>] {
        &self.entries
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the archive declares no entries at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Total decoded bytes counted across every entry.
    #[must_use]
    pub const fn total_decoded_bytes(&self) -> u64 {
        self.total_decoded_bytes
    }

    /// Whether the first entry's raw name is exactly `name`.
    ///
    /// This is an observation, not a verdict: rule A2 expects a `mimetype`
    /// marker entry first, but its directory prefix is unresolved (A19), so the
    /// caller decides what the answer means.
    #[must_use]
    pub fn is_first_entry_named(&self, name: &[u8]) -> bool {
        self.entries
            .first()
            .is_some_and(|entry| entry.name_bytes() == name)
    }

    /// The first entry whose raw name is exactly `name`.
    ///
    /// Names are unique in an accepted archive, byte-exactly and case-folded, so
    /// at most one entry can match.
    #[must_use]
    pub fn find_by_name(&self, name: &[u8]) -> Option<&ArchiveEntry<'a>> {
        self.entries.iter().find(|entry| entry.name_bytes() == name)
    }

    /// Decode one entry again and return its bytes.
    ///
    /// The result is bounded by `max_entry_decoded_bytes`; the archive-wide
    /// total is not re-charged, because the caller asked for this one entry.
    /// Intended for small structural documents such as the format marker and
    /// package metadata, not for attachment payloads.
    ///
    /// The decoded budget is therefore per call, not cumulative: each call is
    /// capped at `max_entry_decoded_bytes` on its own, and `n` calls can decode
    /// up to `n * max_entry_decoded_bytes` in total, unbounded by
    /// `max_total_decoded_bytes`, which only bounds the inventory pass. A caller
    /// that reads several entries budgets for that itself.
    ///
    /// # Errors
    ///
    /// Returns [`ArchiveError::NoSuchEntry`] for an out-of-range index. Decoding
    /// itself cannot fail here: the same entry already decoded once, under the
    /// same limits, while the inventory was built.
    pub fn entry_bytes(&self, index: u32) -> Result<Vec<u8>, ArchiveError> {
        let position =
            usize::try_from(index).map_err(|_| ArchiveError::NoSuchEntry { entry: index })?;
        let entry = self
            .entries
            .get(position)
            .ok_or(ArchiveError::NoSuchEntry { entry: index })?;
        let (start, end) = self.data_ranges[position];
        let capacity = entry
            .uncompressed_size
            .min(self.limits.max_entry_decoded_bytes)
            .min(Limits::OUTPUT_BUFFER_BYTES as u64);
        let mut buffer = Vec::with_capacity(capacity as usize);
        inflate::decode(
            &Decode {
                data: &self.image[start..end],
                method: entry.method,
                declared_size: entry.uncompressed_size,
                declared_crc: entry.crc32,
                entry: index,
                already_decoded: 0,
            },
            &self.limits,
            Sink::Collect(&mut buffer),
        )?;
        Ok(buffer)
    }
}

/// Read a bounded inventory of the archive image in `bytes`.
///
/// The whole image must already be in memory; the core crate never opens a path.
/// Peak additional memory is one inflate state plus one
/// [`Limits::OUTPUT_BUFFER_BYTES`] output buffer, plus per-entry metadata:
/// decoded payloads are counted and discarded, never retained.
///
/// # Errors
///
/// Returns an [`ArchiveError`] whose [`code`](ArchiveError::code) distinguishes
/// truncation, malformed structure, ambiguity, unsupported features, exceeded
/// limits and unsafe entry names. A successful result is not a statement that
/// the archive is a valid KRX package.
///
/// ```
/// use openkrx_core::{Limits, archive};
///
/// let error = archive::inventory(&[], &Limits::DEFAULT).unwrap_err();
/// assert_eq!(error.code(), "archive.malformed.eocd_missing");
/// ```
pub fn inventory<'a>(
    bytes: &'a [u8],
    limits: &Limits,
) -> Result<ArchiveInventory<'a>, ArchiveError> {
    if bytes.len() as u64 > limits.max_archive_bytes {
        return Err(ArchiveError::OverLimit {
            limit: LimitKind::ArchiveBytes,
            limit_value: limits.max_archive_bytes,
            observed: Some(bytes.len() as u64),
            entry: None,
        });
    }
    let end = eocd::find(bytes, limits)?;
    let directory_start = offset(end.directory_offset, Structure::CentralDirectory, None)?;
    let directory = bytes
        .get(directory_start..end.offset)
        .ok_or(ArchiveError::Truncated {
            at: Structure::CentralDirectory,
            entry: None,
        })?;
    let records = central::parse(bytes, directory, end.entries, limits)?;
    let mut layouts = Vec::with_capacity(records.len());
    for (index, record) in records.iter().enumerate() {
        layouts.push(local::verify(bytes, record, index as u32, limits)?);
    }
    check_coverage(&layouts, directory_start)?;
    build(bytes, &records, &layouts, limits)
}

/// Require the image to be exactly covered by the structures it declares.
///
/// Entry regions must start at offset zero, follow one another without a gap or
/// an overlap, and end exactly where the central directory begins. Anything else
/// leaves bytes that belong to no structure, or bytes claimed twice, either of
/// which allows two readers to disagree about the archive's content.
fn check_coverage(layouts: &[EntryLayout], directory_start: usize) -> Result<(), ArchiveError> {
    let malformed = |kind| ArchiveError::Malformed { kind, entry: None };
    let mut ordered: Vec<&EntryLayout> = layouts.iter().collect();
    ordered.sort_unstable_by_key(|layout| layout.header_start);
    let mut cursor = 0_usize;
    for layout in ordered {
        if layout.header_start < cursor {
            return Err(malformed(MalformedKind::OverlappingRanges));
        }
        if layout.header_start > cursor {
            return Err(malformed(if cursor == 0 {
                MalformedKind::PrefixBytes
            } else {
                MalformedKind::UnclaimedBytes
            }));
        }
        cursor = layout.region_end;
    }
    if cursor > directory_start {
        return Err(malformed(MalformedKind::OverlappingRanges));
    }
    if cursor < directory_start {
        return Err(malformed(if cursor == 0 {
            MalformedKind::PrefixBytes
        } else {
            MalformedKind::UnclaimedBytes
        }));
    }
    Ok(())
}

/// Decode every entry, enforcing the aggregate limit, and assemble the result.
fn build<'a>(
    bytes: &'a [u8],
    records: &[CentralRecord<'a>],
    layouts: &[EntryLayout],
    limits: &Limits,
) -> Result<ArchiveInventory<'a>, ArchiveError> {
    let mut entries = Vec::with_capacity(records.len());
    let mut data_ranges = Vec::with_capacity(records.len());
    let mut total_decoded_bytes = 0_u64;
    for (index, (record, layout)) in records.iter().zip(layouts).enumerate() {
        let index = index as u32;
        let decoded_size = inflate::decode(
            &Decode {
                data: &bytes[layout.data_start..layout.data_end],
                method: record.method,
                declared_size: record.uncompressed_size,
                declared_crc: record.crc,
                entry: index,
                already_decoded: total_decoded_bytes,
            },
            limits,
            Sink::Count,
        )?;
        total_decoded_bytes =
            total_decoded_bytes
                .checked_add(decoded_size)
                .ok_or(ArchiveError::OverLimit {
                    limit: LimitKind::TotalDecodedBytes,
                    limit_value: limits.max_total_decoded_bytes,
                    observed: None,
                    entry: Some(index),
                })?;
        data_ranges.push((layout.data_start, layout.data_end));
        entries.push(ArchiveEntry {
            name: record.name,
            name_text: core::str::from_utf8(record.name).ok(),
            utf8_flag: record.utf8_flag(),
            flags: record.flags,
            method: record.method,
            compressed_size: record.compressed_size,
            uncompressed_size: record.uncompressed_size,
            crc32: record.crc,
            local_header_offset: record.local_offset,
            decoded_size,
            version_made_by: record.version_made_by,
            external_attributes: record.external_attributes,
        });
    }
    Ok(ArchiveInventory {
        image: bytes,
        entries,
        data_ranges,
        total_decoded_bytes,
        limits: *limits,
    })
}
