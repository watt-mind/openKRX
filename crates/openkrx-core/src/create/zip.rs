//! The deterministic ZIP image, written record by record.
//!
//! Every field a ZIP writer is free to choose is fixed here, because a
//! difference in any of them would make two runs over the same input produce
//! different bytes:
//!
//! | Field | Value |
//! | --- | --- |
//! | Entry order | marker, metadata document, attachments in index order |
//! | Marker method | stored |
//! | Every other method | deflate, at one fixed level |
//! | Modification date and time | the caller's [`FixedTimestamp`] |
//! | General-purpose flags | bit 11 only: the name is UTF-8 |
//! | `version made by` | host 3 (Unix), version 20 |
//! | `version needed` | 20 |
//! | External attributes | `0o100644 << 16`: a regular file, `rw-r--r--` |
//! | Extra fields, comments, data descriptors | none |
//! | Directory entries | none |
//!
//! No ZIP64 record is ever written, so the image, every entry and every offset
//! must fit in 32 bits. Writing happens in two steps for that reason: an entry
//! is compressed first, the exact image length is known before a byte of it is
//! assembled, and the caller refuses the package when that length exceeds the
//! ceiling — so no size or offset is ever truncated into a header.

use miniz_oxide::deflate::compress_to_vec;

use crate::archive::Crc32;

use super::spec::FixedTimestamp;

/// Local file-header signature.
const LOCAL_SIGNATURE: [u8; 4] = [b'P', b'K', 3, 4];
/// Central-directory file-header signature.
const CENTRAL_SIGNATURE: [u8; 4] = [b'P', b'K', 1, 2];
/// End-of-central-directory signature.
const EOCD_SIGNATURE: [u8; 4] = [b'P', b'K', 5, 6];
/// Stored compression method.
const STORED: u16 = 0;
/// Deflate compression method.
const DEFLATE: u16 = 8;
/// General-purpose bit 11: the entry name is UTF-8.
const FLAG_UTF8: u16 = 1 << 11;
/// `version needed to extract`, and the low byte of `version made by`.
const VERSION: u16 = 20;
/// `version made by`: host 3, the Unix host whose attributes carry `st_mode`.
const VERSION_MADE_BY: u16 = (3 << 8) | VERSION;
/// External attributes: `S_IFREG | 0o644`, in the high 16 bits.
const EXTERNAL_ATTRIBUTES: u32 = 0o100_644 << 16;
/// The one deflate level this crate uses; changing it changes every output.
const DEFLATE_LEVEL: u8 = 6;
/// Fixed bytes of a local file header, before the name.
const LOCAL_HEADER_BYTES: u64 = 30;
/// Fixed bytes of a central-directory record, before the name.
const CENTRAL_RECORD_BYTES: u64 = 46;
/// Fixed bytes of the end-of-central-directory record; the comment is empty.
const EOCD_BYTES: u64 = 22;

/// How an entry's bytes are to be stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Method {
    /// Written verbatim, as rule A2's marker entry conventionally is.
    Stored,
    /// Deflated at the one fixed level.
    Deflate,
}

/// One entry to write, already named and placed.
pub(crate) struct Entry<'a> {
    /// The entry name, exactly as it is to be stored.
    pub(crate) name: &'a str,
    /// The entry's decoded bytes.
    pub(crate) data: &'a [u8],
    /// How to store them.
    pub(crate) method: Method,
}

/// One entry whose stored form and CRC are known.
pub(crate) struct Prepared<'a> {
    name: &'a str,
    method: Method,
    crc: u32,
    stored: Vec<u8>,
    uncompressed_size: u64,
}

/// Compress every entry, so the exact image length is known before assembly.
pub(crate) fn prepare<'a>(entries: &[Entry<'a>]) -> Vec<Prepared<'a>> {
    entries
        .iter()
        .map(|entry| {
            let mut crc = Crc32::new();
            crc.update(entry.data);
            Prepared {
                name: entry.name,
                method: entry.method,
                crc: crc.value(),
                stored: match entry.method {
                    Method::Stored => entry.data.to_vec(),
                    Method::Deflate => compress_to_vec(entry.data, DEFLATE_LEVEL),
                },
                uncompressed_size: entry.data.len() as u64,
            }
        })
        .collect()
}

/// The exact length the assembled image will have, in bytes.
pub(crate) fn image_bytes(prepared: &[Prepared<'_>]) -> u64 {
    prepared
        .iter()
        .map(|entry| {
            let name = entry.name.len() as u64;
            LOCAL_HEADER_BYTES + CENTRAL_RECORD_BYTES + name * 2 + entry.stored.len() as u64
        })
        .sum::<u64>()
        + EOCD_BYTES
}

/// Serialise the image.
///
/// The result is a function of the entries and the timestamp alone: no clock,
/// no environment and no allocation address reaches it. The caller has already
/// checked [`image_bytes`] against the ceiling, so every size and offset here
/// fits in the 32 bits the non-ZIP64 records hold.
pub(crate) fn assemble(prepared: &[Prepared<'_>], timestamp: FixedTimestamp) -> Vec<u8> {
    let length = usize::try_from(image_bytes(prepared)).unwrap_or(0);
    let mut out = Vec::with_capacity(length);
    let mut directory = Vec::new();
    for entry in prepared {
        let offset = out.len() as u32;
        entry.write_local(&mut out, timestamp);
        out.extend_from_slice(&entry.stored);
        entry.write_central(&mut directory, timestamp, offset);
    }
    let directory_offset = out.len() as u32;
    let directory_bytes = directory.len() as u32;
    let count = prepared.len() as u16;
    out.extend_from_slice(&directory);
    out.extend_from_slice(&EOCD_SIGNATURE);
    out.extend_from_slice(&0_u16.to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&directory_bytes.to_le_bytes());
    out.extend_from_slice(&directory_offset.to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes());
    out
}

impl Prepared<'_> {
    /// The compression-method field.
    const fn method(&self) -> u16 {
        match self.method {
            Method::Stored => STORED,
            Method::Deflate => DEFLATE,
        }
    }

    /// The fields both headers must agree on, in the order both write them.
    fn sizes(&self, out: &mut Vec<u8>, timestamp: FixedTimestamp) {
        out.extend_from_slice(&FLAG_UTF8.to_le_bytes());
        out.extend_from_slice(&self.method().to_le_bytes());
        out.extend_from_slice(&timestamp.dos_time().to_le_bytes());
        out.extend_from_slice(&timestamp.dos_date().to_le_bytes());
        out.extend_from_slice(&self.crc.to_le_bytes());
        out.extend_from_slice(&(self.stored.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.uncompressed_size as u32).to_le_bytes());
        out.extend_from_slice(&(self.name.len() as u16).to_le_bytes());
    }

    /// The local file header. The data is appended by the caller, and no extra
    /// field and no data descriptor is ever written.
    fn write_local(&self, out: &mut Vec<u8>, timestamp: FixedTimestamp) {
        out.extend_from_slice(&LOCAL_SIGNATURE);
        out.extend_from_slice(&VERSION.to_le_bytes());
        self.sizes(out, timestamp);
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(self.name.as_bytes());
    }

    /// The central-directory record for the same entry.
    fn write_central(&self, out: &mut Vec<u8>, timestamp: FixedTimestamp, offset: u32) {
        out.extend_from_slice(&CENTRAL_SIGNATURE);
        out.extend_from_slice(&VERSION_MADE_BY.to_le_bytes());
        out.extend_from_slice(&VERSION.to_le_bytes());
        self.sizes(out, timestamp);
        // Extra field, comment, disk number and internal attributes: none.
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&EXTERNAL_ATTRIBUTES.to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        out.extend_from_slice(self.name.as_bytes());
    }
}
