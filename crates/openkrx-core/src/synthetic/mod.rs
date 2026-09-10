//! Test-only synthetic ZIP writer, behind the `synthetic-writer` feature.
//!
//! Every archive the archive, metadata, profile and command-line tests read is
//! generated here, in this repository, from scratch. No third-party or real
//! package is involved, and nothing is committed as a binary fixture. The
//! writer deliberately allows contradictory headers so hostile archives can be
//! constructed exactly.
//!
//! **This module is test scaffolding, not a supported API.** The
//! `synthetic-writer` feature is off by default, the `openkrx` executable
//! never enables it, and no shipped binary contains this code. It exists in
//! `src/` rather than in `tests/` only so that the command-line crate can
//! build the same archives from its own subprocess tests without duplicating
//! the writer or committing a fixture.

pub mod meta;

use miniz_oxide::deflate::compress_to_vec;

/// Local file-header signature.
pub const LOCAL_SIGNATURE: [u8; 4] = [b'P', b'K', 3, 4];
/// Central-directory file-header signature.
pub const CENTRAL_SIGNATURE: [u8; 4] = [b'P', b'K', 1, 2];
/// End-of-central-directory signature.
pub const EOCD_SIGNATURE: [u8; 4] = [b'P', b'K', 5, 6];
/// Data-descriptor signature.
pub const DESCRIPTOR_SIGNATURE: [u8; 4] = [b'P', b'K', 7, 8];
/// Stored compression method.
pub const STORED: u16 = 0;
/// Deflate compression method.
pub const DEFLATE: u16 = 8;
/// General-purpose bit 3.
pub const FLAG_DESCRIPTOR: u16 = 1 << 3;
/// General-purpose bit 11.
pub const FLAG_UTF8: u16 = 1 << 11;

/// CRC-32 over `data`, computed independently of the crate under test.
#[must_use]
pub fn crc32(data: &[u8]) -> u32 {
    let mut state = !0_u32;
    for byte in data {
        state ^= u32::from(*byte);
        for _ in 0..8 {
            state = if state & 1 == 1 {
                0xedb8_8320 ^ (state >> 1)
            } else {
                state >> 1
            };
        }
    }
    !state
}

/// One entry, with optional deliberate contradictions between its headers.
#[derive(Clone)]
pub struct Entry {
    /// Name written into the central directory.
    pub name: Vec<u8>,
    /// Bytes written as the entry's data, already compressed if `method` says so.
    pub data: Vec<u8>,
    /// Uncompressed size written into the headers.
    pub uncompressed_size: u32,
    /// CRC-32 written into the headers.
    pub crc: u32,
    /// Compression method.
    pub method: u16,
    /// General-purpose bit flags.
    pub flags: u16,
    /// Extra field written into the local header.
    pub local_extra: Vec<u8>,
    /// Extra field written into the central-directory record.
    pub central_extra: Vec<u8>,
    /// Comment written into the central-directory record.
    pub central_comment: Vec<u8>,
    /// Disk number written into the central-directory record.
    pub disk_start: u16,
    /// Compressed size override for the central-directory record.
    pub central_compressed: Option<u32>,
    /// Uncompressed size override for the central-directory record.
    pub central_uncompressed: Option<u32>,
    /// CRC override for the central-directory record.
    pub central_crc: Option<u32>,
    /// Method override for the central-directory record.
    pub central_method: Option<u16>,
    /// Local-header name override.
    pub local_name: Option<Vec<u8>>,
    /// Local-header flags override.
    pub local_flags: Option<u16>,
    /// Local-header method override.
    pub local_method: Option<u16>,
    /// Local-header CRC override.
    pub local_crc: Option<u32>,
    /// Local-header compressed-size override.
    pub local_compressed: Option<u32>,
    /// Local-header uncompressed-size override.
    pub local_uncompressed: Option<u32>,
    /// Local-header signature override.
    pub local_signature: Option<[u8; 4]>,
    /// Central-directory signature override.
    pub central_signature: Option<[u8; 4]>,
    /// Raw bytes appended after the data, used for data descriptors.
    pub trailer: Vec<u8>,
    /// Local-header offset override written into the central directory.
    pub local_offset: Option<u32>,
    /// `version made by` written into the central-directory record; its high
    /// byte is the host system that decides how `external_attributes` reads.
    pub version_made_by: u16,
    /// `external file attributes` written into the central-directory record.
    pub external_attributes: u32,
}

impl Entry {
    /// A stored entry holding `data` verbatim.
    #[must_use]
    pub fn stored(name: &[u8], data: &[u8]) -> Self {
        Self {
            name: name.to_vec(),
            data: data.to_vec(),
            uncompressed_size: u32::try_from(data.len()).expect("test data fits in u32"),
            crc: crc32(data),
            method: STORED,
            flags: 0,
            local_extra: Vec::new(),
            central_extra: Vec::new(),
            central_comment: Vec::new(),
            disk_start: 0,
            central_compressed: None,
            central_uncompressed: None,
            central_crc: None,
            central_method: None,
            local_name: None,
            local_flags: None,
            local_method: None,
            local_crc: None,
            local_compressed: None,
            local_uncompressed: None,
            local_signature: None,
            central_signature: None,
            trailer: Vec::new(),
            local_offset: None,
            version_made_by: 20,
            external_attributes: 0,
        }
    }

    /// A deflated entry whose decoded content is `data`.
    ///
    /// Compressed at the one level this crate's writers emit, so the entry is
    /// the shape a package `create::package` wrote would carry.
    #[must_use]
    pub fn deflated(name: &[u8], data: &[u8]) -> Self {
        Self::deflated_stream(name, data, compress_to_vec(data, 6))
    }

    /// A deflated entry holding `compressed`, declaring `data` as its content.
    ///
    /// [`Entry::deflated`] compresses at one level, which fixes the deflate
    /// block types the reader ever sees from it. This constructor takes the
    /// compressed bytes instead, so a test can put a stream the writers would
    /// never produce — another level, a stored or a fixed-Huffman block, a
    /// stream from another compressor — in front of the reader.
    ///
    /// The CRC-32 and the declared size are still derived from `data`, so the
    /// entry is well formed exactly when `compressed` is `data` deflated. A
    /// caller that hands it anything else is building a deliberately broken
    /// entry, which is a legitimate thing to want here.
    #[must_use]
    pub fn deflated_stream(name: &[u8], data: &[u8], compressed: Vec<u8>) -> Self {
        let mut entry = Self::stored(name, data);
        entry.data = compressed;
        entry.method = DEFLATE;
        entry
    }

    /// Declare the entry with a Unix host system and `mode` as its `st_mode`.
    ///
    /// This is how an archive states that an entry is a symbolic link, a
    /// device node or a directory, so it is how a test builds one.
    #[must_use]
    pub const fn with_unix_mode(mut self, mode: u32) -> Self {
        self.version_made_by = (3 << 8) | 20;
        self.external_attributes = mode << 16;
        self
    }

    /// Declare the entry with an MS-DOS host system and `attributes` as its
    /// FAT attribute bits, of which bit 4 (`0x10`) marks a directory.
    #[must_use]
    pub const fn with_dos_attributes(mut self, attributes: u32) -> Self {
        self.version_made_by = 20;
        self.external_attributes = attributes;
        self
    }

    /// Declare the entry with a host system this reader does not map.
    #[must_use]
    pub const fn with_host_system(mut self, host: u16) -> Self {
        self.version_made_by = (host << 8) | 20;
        self
    }

    /// Attach a data descriptor that repeats the entry's own values.
    #[must_use]
    pub fn with_descriptor(mut self, signed: bool) -> Self {
        self.flags |= FLAG_DESCRIPTOR;
        self.local_crc = Some(0);
        self.local_compressed = Some(0);
        self.local_uncompressed = Some(0);
        let mut trailer = Vec::new();
        if signed {
            trailer.extend_from_slice(&DESCRIPTOR_SIGNATURE);
        }
        trailer.extend_from_slice(&self.crc.to_le_bytes());
        let compressed = u32::try_from(self.data.len()).expect("test data fits in u32");
        trailer.extend_from_slice(&compressed.to_le_bytes());
        trailer.extend_from_slice(&self.uncompressed_size.to_le_bytes());
        self.trailer = trailer;
        self
    }

    /// Bytes the entry's local record occupies.
    fn local_record(&self) -> Vec<u8> {
        let name = self.local_name.as_ref().unwrap_or(&self.name);
        let mut out = Vec::new();
        out.extend_from_slice(&self.local_signature.unwrap_or(LOCAL_SIGNATURE));
        out.extend_from_slice(&20_u16.to_le_bytes());
        out.extend_from_slice(&self.local_flags.unwrap_or(self.flags).to_le_bytes());
        out.extend_from_slice(&self.local_method.unwrap_or(self.method).to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&self.local_crc.unwrap_or(self.crc).to_le_bytes());
        let compressed = u32::try_from(self.data.len()).expect("test data fits in u32");
        out.extend_from_slice(&self.local_compressed.unwrap_or(compressed).to_le_bytes());
        out.extend_from_slice(
            &self
                .local_uncompressed
                .unwrap_or(self.uncompressed_size)
                .to_le_bytes(),
        );
        out.extend_from_slice(&len16(name).to_le_bytes());
        out.extend_from_slice(&len16(&self.local_extra).to_le_bytes());
        out.extend_from_slice(name);
        out.extend_from_slice(&self.local_extra);
        out.extend_from_slice(&self.data);
        out.extend_from_slice(&self.trailer);
        out
    }

    /// Bytes the entry's central-directory record occupies.
    fn central_record(&self, offset: u32) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.central_signature.unwrap_or(CENTRAL_SIGNATURE));
        out.extend_from_slice(&self.version_made_by.to_le_bytes());
        out.extend_from_slice(&20_u16.to_le_bytes());
        out.extend_from_slice(&self.flags.to_le_bytes());
        out.extend_from_slice(&self.central_method.unwrap_or(self.method).to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&self.central_crc.unwrap_or(self.crc).to_le_bytes());
        let compressed = u32::try_from(self.data.len()).expect("test data fits in u32");
        out.extend_from_slice(&self.central_compressed.unwrap_or(compressed).to_le_bytes());
        out.extend_from_slice(
            &self
                .central_uncompressed
                .unwrap_or(self.uncompressed_size)
                .to_le_bytes(),
        );
        out.extend_from_slice(&len16(&self.name).to_le_bytes());
        out.extend_from_slice(&len16(&self.central_extra).to_le_bytes());
        out.extend_from_slice(&len16(&self.central_comment).to_le_bytes());
        out.extend_from_slice(&self.disk_start.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&self.external_attributes.to_le_bytes());
        out.extend_from_slice(&self.local_offset.unwrap_or(offset).to_le_bytes());
        out.extend_from_slice(&self.name);
        out.extend_from_slice(&self.central_extra);
        out.extend_from_slice(&self.central_comment);
        out
    }
}

/// Assemble an archive image from entries and optional whole-archive faults.
#[derive(Clone, Default)]
pub struct Archive {
    /// Entries, written in this order both locally and centrally.
    pub entries: Vec<Entry>,
    /// Bytes written before the first local header; offsets are adjusted.
    pub prefix: Vec<u8>,
    /// Bytes appended after the end-of-central-directory comment.
    pub suffix: Vec<u8>,
    /// End-of-central-directory comment.
    pub comment: Vec<u8>,
    /// Entry-count override in the end record.
    pub eocd_entries: Option<u16>,
    /// Central-directory size override in the end record.
    pub eocd_directory_bytes: Option<u32>,
    /// Central-directory offset override in the end record.
    pub eocd_directory_offset: Option<u32>,
    /// Disk-number override in the end record.
    pub eocd_disk: Option<u16>,
    /// Directory-start-disk override in the end record.
    pub eocd_directory_disk: Option<u16>,
    /// Entries-on-this-disk override in the end record.
    pub eocd_entries_here: Option<u16>,
}

impl Archive {
    /// An archive holding exactly these entries and nothing unusual.
    #[must_use]
    pub fn of(entries: Vec<Entry>) -> Self {
        Self {
            entries,
            ..Self::default()
        }
    }

    /// Serialise the archive image.
    #[must_use]
    pub fn build(&self) -> Vec<u8> {
        let mut out = self.prefix.clone();
        let mut offsets = Vec::new();
        for entry in &self.entries {
            offsets.push(u32::try_from(out.len()).expect("test archive fits in u32"));
            out.extend_from_slice(&entry.local_record());
        }
        let directory_offset = u32::try_from(out.len()).expect("test archive fits in u32");
        for (entry, offset) in self.entries.iter().zip(&offsets) {
            out.extend_from_slice(&entry.central_record(*offset));
        }
        let directory_bytes =
            u32::try_from(out.len()).expect("test archive fits in u32") - directory_offset;
        let count = u16::try_from(self.entries.len()).expect("test entry count fits in u16");
        out.extend_from_slice(&EOCD_SIGNATURE);
        out.extend_from_slice(&self.eocd_disk.unwrap_or(0).to_le_bytes());
        out.extend_from_slice(&self.eocd_directory_disk.unwrap_or(0).to_le_bytes());
        let total = self.eocd_entries.unwrap_or(count);
        out.extend_from_slice(&self.eocd_entries_here.unwrap_or(total).to_le_bytes());
        out.extend_from_slice(&total.to_le_bytes());
        out.extend_from_slice(
            &self
                .eocd_directory_bytes
                .unwrap_or(directory_bytes)
                .to_le_bytes(),
        );
        out.extend_from_slice(
            &self
                .eocd_directory_offset
                .unwrap_or(directory_offset)
                .to_le_bytes(),
        );
        out.extend_from_slice(&len16(&self.comment).to_le_bytes());
        out.extend_from_slice(&self.comment);
        out.extend_from_slice(&self.suffix);
        out
    }
}

/// A minimal, well-formed two-entry archive shaped like a KRX container.
///
/// The shape is an observation aid only: `docs/profile.md` rules A19 to A22 are
/// unresolved, so nothing about this layout is asserted to be conforming.
#[must_use]
pub fn marker_archive() -> Vec<u8> {
    Archive::of(vec![
        Entry::stored(b"mimetype", b"application/OCD+ZIP"),
        Entry::deflated(b"Metalayer/KULDEMENY_META.xml", b"<KULDEMENY/>"),
    ])
    .build()
}

/// Assemble an image from raw parts, with a consistent end record.
///
/// Used where a central directory must be malformed in a way the structured
/// builder cannot express, such as a record cut short.
#[must_use]
pub fn assemble(body: &[u8], directory: &[u8], entries: u16) -> Vec<u8> {
    let mut out = body.to_vec();
    let directory_offset = u32::try_from(out.len()).expect("test archive fits in u32");
    out.extend_from_slice(directory);
    out.extend_from_slice(&EOCD_SIGNATURE);
    out.extend_from_slice(&0_u16.to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes());
    out.extend_from_slice(&entries.to_le_bytes());
    out.extend_from_slice(&entries.to_le_bytes());
    let directory_bytes = u32::try_from(directory.len()).expect("test directory fits in u32");
    out.extend_from_slice(&directory_bytes.to_le_bytes());
    out.extend_from_slice(&directory_offset.to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes());
    out
}

/// Split a built image into everything before, and all of, its central directory.
///
/// The split point is taken from the end record, so it stays correct whatever
/// the entries look like.
#[must_use]
pub fn split_directory(image: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let eocd = image.len() - 22;
    let offset = u32::from_le_bytes([
        image[eocd + 16],
        image[eocd + 17],
        image[eocd + 18],
        image[eocd + 19],
    ]) as usize;
    (image[..offset].to_vec(), image[offset..eocd].to_vec())
}

/// Deterministic, poorly compressible bytes for realistic payload tests.
#[must_use]
pub fn pseudo_random(length: usize) -> Vec<u8> {
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    (0..length)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 24) as u8
        })
        .collect()
}

/// Length of `bytes` as a `u16` header field.
fn len16(bytes: &[u8]) -> u16 {
    u16::try_from(bytes.len()).expect("test field fits in u16")
}
