//! Local file header and data-descriptor checking.
//!
//! The central directory is authoritative. A local header must agree with its
//! record on name, flags, method, CRC and both sizes; a data descriptor is
//! accepted only when general-purpose bit 3 is set and the descriptor repeats
//! the central values exactly.

use crate::error::{ArchiveError, LimitKind, MalformedKind, Structure};
use crate::limits::Limits;

use super::central::CentralRecord;
use super::raw::{Cursor, offset};

/// Local file-header signature.
const LOCAL_SIGNATURE: u32 = 0x0403_4b50;
/// Optional data-descriptor signature.
const DESCRIPTOR_SIGNATURE: u32 = 0x0807_4b50;

/// The byte ranges one entry claims in the archive image.
#[derive(Debug, Clone, Copy)]
pub(crate) struct EntryLayout {
    /// Offset of the local file header.
    pub(crate) header_start: usize,
    /// Offset of the entry's compressed data.
    pub(crate) data_start: usize,
    /// End of the entry's compressed data.
    pub(crate) data_end: usize,
    /// End of everything the entry claims, including any data descriptor.
    pub(crate) region_end: usize,
}

/// Verify the local header for `record` and return the ranges it claims.
pub(crate) fn verify(
    bytes: &[u8],
    record: &CentralRecord<'_>,
    index: u32,
    limits: &Limits,
) -> Result<EntryLayout, ArchiveError> {
    let entry = Some(index);
    let mismatch = || ArchiveError::Malformed {
        kind: MalformedKind::LocalHeaderMismatch,
        entry,
    };
    let header_start = offset(record.local_offset, Structure::LocalHeader, entry)?;
    let tail = bytes.get(header_start..).ok_or(ArchiveError::Truncated {
        at: Structure::LocalHeader,
        entry,
    })?;
    let mut cursor = Cursor::new(tail, Structure::LocalHeader, entry);
    cursor.signature(LOCAL_SIGNATURE)?;
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
    if u64::from(extra_bytes) > u64::from(limits.max_extra_field_bytes) {
        return Err(ArchiveError::OverLimit {
            limit: LimitKind::ExtraFieldBytes,
            limit_value: u64::from(limits.max_extra_field_bytes),
            observed: Some(u64::from(extra_bytes)),
            entry,
        });
    }
    let name = cursor.take(usize::from(name_bytes))?;
    cursor.take(usize::from(extra_bytes))?;

    if flags != record.flags || method != record.method || name != record.name {
        return Err(mismatch());
    }
    let declared = (
        u64::from(crc),
        u64::from(compressed_size),
        u64::from(uncompressed_size),
    );
    let central = (
        u64::from(record.crc),
        record.compressed_size,
        record.uncompressed_size,
    );
    if record.has_data_descriptor() {
        if declared != (0, 0, 0) && declared != central {
            return Err(mismatch());
        }
    } else if declared != central {
        return Err(mismatch());
    }

    let data_start = header_start
        .checked_add(cursor.position())
        .ok_or_else(mismatch)?;
    let data_length = offset(record.compressed_size, Structure::EntryData, entry)?;
    let data_end = data_start
        .checked_add(data_length)
        .ok_or(ArchiveError::Truncated {
            at: Structure::EntryData,
            entry,
        })?;
    if data_end > bytes.len() {
        return Err(ArchiveError::Truncated {
            at: Structure::EntryData,
            entry,
        });
    }
    let region_end = if record.has_data_descriptor() {
        verify_descriptor(bytes, record, data_end, index)?
    } else {
        data_end
    };
    Ok(EntryLayout {
        header_start,
        data_start,
        data_end,
        region_end,
    })
}

/// Check the data descriptor that follows an entry written with bit 3 set.
///
/// The optional descriptor signature is honoured when present. A descriptor
/// whose first word equals the signature value while actually being a CRC is
/// indistinguishable in the format itself; such an archive is read in the
/// signed form and then fails the value comparison.
fn verify_descriptor(
    bytes: &[u8],
    record: &CentralRecord<'_>,
    data_end: usize,
    index: u32,
) -> Result<usize, ArchiveError> {
    let entry = Some(index);
    let tail = bytes.get(data_end..).ok_or(ArchiveError::Truncated {
        at: Structure::DataDescriptor,
        entry,
    })?;
    let mut cursor = Cursor::new(tail, Structure::DataDescriptor, entry);
    let first = cursor.u32()?;
    let crc = if first == DESCRIPTOR_SIGNATURE {
        cursor.u32()?
    } else {
        first
    };
    let compressed_size = cursor.u32()?;
    let uncompressed_size = cursor.u32()?;
    if crc != record.crc
        || u64::from(compressed_size) != record.compressed_size
        || u64::from(uncompressed_size) != record.uncompressed_size
    {
        return Err(ArchiveError::Malformed {
            kind: MalformedKind::DataDescriptorMismatch,
            entry,
        });
    }
    data_end
        .checked_add(cursor.position())
        .ok_or(ArchiveError::Truncated {
            at: Structure::DataDescriptor,
            entry,
        })
}
