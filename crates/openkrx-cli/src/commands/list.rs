//! `list`: every archive entry, in central-directory order.
//!
//! The command is profile-agnostic, exactly as
//! `openkrx_core::archive::inventory` is: it says what the ZIP image contains
//! and nothing about KRX. Every number comes from the inventory, so a declared
//! size and the size actually decoded are reported side by side rather than
//! collapsed into one figure.
//!
//! Output is bounded by the inventory itself: `Limits::DEFAULT.max_entries` is
//! 256, so a list can never be longer than that, and each name is bounded by
//! `max_name_bytes`.

use openkrx_core::archive::{ArchiveEntry, ArchiveInventory};
use serde::Serialize;

use super::{hex_when_not_utf8, text};

/// What `list` reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ListData {
    /// Every entry, in central-directory order.
    pub entries: Vec<EntryView>,
}

/// One archive entry as the inventory observed it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EntryView {
    /// Central-directory index, counted from zero.
    pub index: u32,
    /// The name, present when the bytes are valid UTF-8.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Whether the entry set general-purpose bit 11, the UTF-8 name flag.
    ///
    /// Reported independently of whether the bytes decode: a name can be
    /// valid UTF-8 without the flag, and the flag can be set on bytes that
    /// are not.
    pub name_utf8_flag: bool,
    /// Lower-case hexadecimal of the name bytes, present only when the name
    /// is not valid UTF-8.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_hex: Option<String>,
    /// `stored` or `deflate`; no other method reaches the inventory.
    pub method: &'static str,
    /// Compressed size, as the central directory declares it.
    pub compressed_size: u64,
    /// Uncompressed size, as the central directory declares it.
    pub declared_size: u64,
    /// Bytes the decoder actually produced.
    pub decoded_size: u64,
    /// The declared CRC-32, as eight lower-case hexadecimal digits.
    ///
    /// The inventory already checked the decoded bytes against it: a mismatch
    /// is `archive.malformed.crc_mismatch` and never reaches this report. A
    /// CRC detects corruption and says nothing about authenticity.
    pub crc32: String,
    /// The raw name bytes, for terminal-safe rendering. Never serialised.
    #[serde(skip)]
    pub name_bytes: Vec<u8>,
}

/// Build the report from an inventory.
#[must_use]
pub fn run(inventory: &ArchiveInventory<'_>) -> ListData {
    ListData {
        entries: inventory
            .entries()
            .iter()
            .enumerate()
            .map(|(index, entry)| view(index, entry))
            .collect(),
    }
}

/// One entry's view; `index` is its position in the central directory.
fn view(index: usize, entry: &ArchiveEntry<'_>) -> EntryView {
    let name = entry.name_bytes();
    EntryView {
        index: u32::try_from(index).unwrap_or(u32::MAX),
        name: text(name),
        name_utf8_flag: entry.utf8_flag(),
        name_hex: hex_when_not_utf8(name),
        method: method(entry.method()),
        compressed_size: entry.compressed_size(),
        declared_size: entry.uncompressed_size(),
        decoded_size: entry.decoded_size(),
        crc32: format!("{:08x}", entry.crc32()),
        name_bytes: name.to_vec(),
    }
}

/// The two methods the inventory accepts; it refuses every other one.
fn method(method: u16) -> &'static str {
    match method {
        0 => "stored",
        8 => "deflate",
        // Unreachable: `archive.unsupported.method` refuses anything else
        // before an inventory exists.
        _ => "unknown",
    }
}
