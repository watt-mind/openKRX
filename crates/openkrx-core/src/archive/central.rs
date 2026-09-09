//! Central-directory parsing and per-record validation.
//!
//! The central directory is authoritative for entry metadata; local headers are
//! checked against it rather than the other way round. Every record is validated
//! for supported features, name safety, name collisions and declared limits
//! before any entry data is touched.

use crate::error::{ArchiveError, LimitKind, MalformedKind, Structure, UnsupportedKind};
use crate::limits::Limits;

use super::inflate::{METHOD_DEFLATE, METHOD_STORED};
use super::names;
use super::raw::Cursor;

/// Central-directory file-header signature.
const CENTRAL_SIGNATURE: u32 = 0x0201_4b50;
/// ZIP64 extended-information extra-field header id.
const ZIP64_EXTRA_ID: u16 = 0x0001;
/// General-purpose bit 0: the entry is encrypted.
const FLAG_ENCRYPTED: u16 = 1 << 0;
/// General-purpose bit 3: sizes and CRC follow the data in a descriptor.
pub(crate) const FLAG_DATA_DESCRIPTOR: u16 = 1 << 3;
/// General-purpose bit 5: the entry holds compressed patched data.
const FLAG_PATCHED: u16 = 1 << 5;
/// General-purpose bit 6: strong encryption.
const FLAG_STRONG_ENCRYPTION: u16 = 1 << 6;
/// General-purpose bit 11: the name is declared to be UTF-8.
const FLAG_UTF8: u16 = 1 << 11;
/// General-purpose bit 13: local header values are masked by strong encryption.
const FLAG_MASKED_HEADER: u16 = 1 << 13;

/// One validated central-directory record.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CentralRecord<'a> {
    /// Raw name bytes, exactly as stored.
    pub(crate) name: &'a [u8],
    /// General-purpose bit flags.
    pub(crate) flags: u16,
    /// Compression method.
    pub(crate) method: u16,
    /// Declared CRC-32 of the decoded data.
    pub(crate) crc: u32,
    /// Declared compressed size.
    pub(crate) compressed_size: u64,
    /// Declared uncompressed size.
    pub(crate) uncompressed_size: u64,
    /// Offset of the matching local file header.
    pub(crate) local_offset: u64,
    /// `version made by`: the high byte names the host system.
    pub(crate) version_made_by: u16,
    /// `external file attributes`, interpreted per host system.
    pub(crate) external_attributes: u32,
}

impl CentralRecord<'_> {
    /// Whether the record declares its name to be UTF-8.
    pub(crate) const fn utf8_flag(&self) -> bool {
        self.flags & FLAG_UTF8 != 0
    }

    /// Whether the record's data is followed by a data descriptor.
    pub(crate) const fn has_data_descriptor(&self) -> bool {
        self.flags & FLAG_DATA_DESCRIPTOR != 0
    }
}

/// Parse every central-directory record declared by the end record.
pub(crate) fn parse<'a>(
    bytes: &'a [u8],
    directory: &'a [u8],
    entries: u32,
    limits: &Limits,
) -> Result<Vec<CentralRecord<'a>>, ArchiveError> {
    let mut records = Vec::new();
    let mut seen: Vec<(&[u8], Vec<u8>)> = Vec::new();
    let mut cursor = Cursor::new(directory, Structure::CentralDirectory, None);
    for index in 0..entries {
        if cursor.position() == directory.len() {
            return Err(ArchiveError::Malformed {
                kind: MalformedKind::CentralDirectoryCount,
                entry: Some(index),
            });
        }
        let record = parse_one(&mut cursor, index, limits, bytes.len())?;
        let folded = names::fold_name(record.name);
        names::check_collision(&seen, record.name, &folded, index)?;
        seen.push((record.name, folded));
        records.push(record);
    }
    if cursor.position() == directory.len() {
        Ok(records)
    } else {
        Err(ArchiveError::Malformed {
            kind: MalformedKind::CentralDirectorySize,
            entry: None,
        })
    }
}

/// Parse and validate a single record at the cursor's position.
fn parse_one<'a>(
    cursor: &mut Cursor<'a>,
    index: u32,
    limits: &Limits,
    image_bytes: usize,
) -> Result<CentralRecord<'a>, ArchiveError> {
    cursor.signature(CENTRAL_SIGNATURE)?;
    let version_made_by = cursor.u16()?;
    let _version_needed = cursor.u16()?;
    let flags = cursor.u16()?;
    let method = cursor.u16()?;
    let _modified_time = cursor.u16()?;
    let _modified_date = cursor.u16()?;
    let crc = cursor.u32()?;
    let compressed_size = cursor.u32()?;
    let uncompressed_size = cursor.u32()?;
    let name_bytes = cursor.u16()?;
    let extra_bytes = cursor.u16()?;
    let comment_bytes = cursor.u16()?;
    let disk = cursor.u16()?;
    let _internal_attributes = cursor.u16()?;
    let external_attributes = cursor.u32()?;
    let local_offset = cursor.u32()?;

    check_flags(flags, index)?;
    check_method(method, index)?;
    if disk != 0 {
        return Err(unsupported(UnsupportedKind::MultiDisk, None, index));
    }
    if compressed_size == u32::MAX || uncompressed_size == u32::MAX || local_offset == u32::MAX {
        return Err(unsupported(UnsupportedKind::Zip64, None, index));
    }
    check_limit(
        u64::from(name_bytes),
        u64::from(limits.max_name_bytes),
        LimitKind::NameBytes,
        index,
    )?;
    check_limit(
        u64::from(extra_bytes),
        u64::from(limits.max_extra_field_bytes),
        LimitKind::ExtraFieldBytes,
        index,
    )?;
    check_limit(
        u64::from(comment_bytes),
        u64::from(limits.max_comment_bytes),
        LimitKind::CommentBytes,
        index,
    )?;

    let name = cursor.take(usize::from(name_bytes))?;
    let extra = cursor.take(usize::from(extra_bytes))?;
    cursor.take(usize::from(comment_bytes))?;
    if has_zip64_extra(extra) {
        return Err(unsupported(UnsupportedKind::Zip64, None, index));
    }
    names::check_name(name, index, uncompressed_size != 0 || compressed_size != 0)?;
    if method == METHOD_STORED && compressed_size != uncompressed_size {
        return Err(ArchiveError::Malformed {
            kind: MalformedKind::DeclaredSizeMismatch,
            entry: Some(index),
        });
    }
    if u64::from(compressed_size) > image_bytes as u64 {
        return Err(ArchiveError::Truncated {
            at: Structure::EntryData,
            entry: Some(index),
        });
    }
    Ok(CentralRecord {
        name,
        flags,
        method,
        crc,
        compressed_size: u64::from(compressed_size),
        uncompressed_size: u64::from(uncompressed_size),
        local_offset: u64::from(local_offset),
        version_made_by,
        external_attributes,
    })
}

/// Reject general-purpose flags this reader does not implement.
fn check_flags(flags: u16, index: u32) -> Result<(), ArchiveError> {
    if flags & (FLAG_ENCRYPTED | FLAG_STRONG_ENCRYPTION | FLAG_MASKED_HEADER) != 0 {
        return Err(unsupported(
            UnsupportedKind::Encryption,
            Some(u64::from(flags)),
            index,
        ));
    }
    if flags & FLAG_PATCHED != 0 {
        return Err(unsupported(
            UnsupportedKind::PatchedData,
            Some(u64::from(flags)),
            index,
        ));
    }
    Ok(())
}

/// Reject any compression method other than stored or deflate.
fn check_method(method: u16, index: u32) -> Result<(), ArchiveError> {
    if method == METHOD_STORED || method == METHOD_DEFLATE {
        Ok(())
    } else {
        Err(unsupported(
            UnsupportedKind::Method,
            Some(u64::from(method)),
            index,
        ))
    }
}

/// Report a limit breach with the observed and configured values.
fn check_limit(
    observed: u64,
    limit_value: u64,
    limit: LimitKind,
    index: u32,
) -> Result<(), ArchiveError> {
    if observed > limit_value {
        return Err(ArchiveError::OverLimit {
            limit,
            limit_value,
            observed: Some(observed),
            entry: Some(index),
        });
    }
    Ok(())
}

/// Detect a ZIP64 extended-information field inside an extra-field block.
fn has_zip64_extra(mut extra: &[u8]) -> bool {
    while extra.len() >= 4 {
        let id = u16::from_le_bytes([extra[0], extra[1]]);
        let size = usize::from(u16::from_le_bytes([extra[2], extra[3]]));
        if id == ZIP64_EXTRA_ID {
            return true;
        }
        let Some(rest) = extra.get(4 + size..) else {
            return false;
        };
        extra = rest;
    }
    false
}

/// Build an entry-scoped unsupported-feature error.
const fn unsupported(kind: UnsupportedKind, value: Option<u64>, index: u32) -> ArchiveError {
    ArchiveError::Unsupported {
        kind,
        value,
        entry: Some(index),
    }
}
