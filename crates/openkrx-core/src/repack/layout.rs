//! Reading an archive as the canonical documented layout, or refusing it.
//!
//! Repacking writes with [`crate::create::package`], which emits one layout:
//! the marker, the metadata document, then one entry per attachment under
//! `KRX/OCD/Payload/ID-<n>/`. A package that is not already in that shape
//! cannot be re-emitted in it without moving, renaming or dropping something,
//! and rules A19, A22 and M12 are exactly what leave the other shapes open. So
//! this module recognises the canonical layout byte-exactly and refuses
//! everything else with a code that says which part did not match.
//!
//! Nothing here is a conformance statement, in either direction: a package this
//! module refuses is not malformed, and one it accepts is not conforming.

use crate::archive::ArchiveInventory;
use crate::create::{MARKER_CONTENT, MARKER_NAME, METADATA_NAME, PAYLOAD_PREFIX};

use super::error::{RepackError, UnsupportedKind};

/// The metadata document's directory segment (A4).
const METALAYER: &[u8] = b"Metalayer";
/// The metadata document's file name (A4, M12).
const METADATA_FILE: &[u8] = b"KULDEMENY_META.xml";
/// The canonical root prefix (A19).
const ROOT_PREFIX: &[u8] = b"KRX/OCD/";

/// One payload entry of a canonical package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Payload {
    /// Central-directory index of the entry.
    pub(super) entry: u32,
    /// The file name inside `KRX/OCD/Payload/ID-<n>/`.
    pub(super) file_name: String,
    /// The CRC-32 the archive declares for it, used to recognise the same
    /// inventory again when a plan is applied.
    pub(super) crc32: u32,
}

/// What a canonical package holds, entry by entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Canonical {
    /// Central-directory index of the metadata document; always 1.
    pub(super) metadata_entry: u32,
    /// The payload entries, in attachment-number order.
    pub(super) payloads: Vec<Payload>,
}

/// Refuse with `kind`, naming the entry it concerns.
fn refuse(kind: UnsupportedKind, entry: Option<u32>) -> RepackError {
    RepackError::Unsupported {
        kind,
        entry,
        number: None,
    }
}

/// Read `inventory` as the canonical documented layout.
///
/// The checks run in a fixed order — the metadata document's presence, its
/// prefix, its spelling, the marker, the document's position, then every
/// remaining entry — so one package always reports the same code.
///
/// # Errors
///
/// A `repack.unsupported.*` [`RepackError`] naming the first part of the
/// layout that did not match.
pub(super) fn read(inventory: &ArchiveInventory<'_>) -> Result<Canonical, RepackError> {
    let located = locate(inventory)?;
    let name = inventory.entries()[located as usize].name_bytes();
    // The two facts are separated on purpose: everything before `Metalayer/`
    // is the prefix rule A19 leaves open, and the two segments after it are
    // the spelling rule M12 leaves open. A caller needs to know which of them
    // the package differs in.
    let prefix_length = name.len() - (METALAYER.len() + 1 + METADATA_FILE.len());
    if &name[..prefix_length] != ROOT_PREFIX {
        return Err(refuse(UnsupportedKind::RootPrefix, Some(located)));
    }
    if name != METADATA_NAME.as_bytes() {
        return Err(refuse(UnsupportedKind::MetadataName, Some(located)));
    }
    marker(inventory)?;
    if located != 1 {
        return Err(refuse(UnsupportedKind::ExtraEntry, Some(located)));
    }
    Ok(Canonical {
        metadata_entry: located,
        payloads: payloads(inventory)?,
    })
}

/// The single entry shaped like a metadata document.
///
/// The shape is the one `crate::profile` recognises — the last two segments
/// are `Metalayer/KULDEMENY_META.xml`, compared case-insensitively, under any
/// number of leading segments — so a package whose only defect is a prefix or
/// a spelling is refused with the code that says so, rather than with
/// "no metadata document".
fn locate(inventory: &ArchiveInventory<'_>) -> Result<u32, RepackError> {
    let mut found: Option<u32> = None;
    for (index, entry) in inventory.entries().iter().enumerate() {
        if !metadata_shaped(entry.name_bytes()) {
            continue;
        }
        if found.is_some() {
            return Err(refuse(UnsupportedKind::MetadataMissing, None));
        }
        found = u32::try_from(index).ok();
    }
    found.ok_or_else(|| refuse(UnsupportedKind::MetadataMissing, None))
}

/// Whether `name`'s last two segments name a metadata document (A4, M12).
fn metadata_shaped(name: &[u8]) -> bool {
    let Some(file_start) = name.iter().rposition(|byte| *byte == b'/') else {
        return false;
    };
    if !name[file_start + 1..].eq_ignore_ascii_case(METADATA_FILE) {
        return false;
    }
    let head = &name[..file_start];
    let directory_start = head
        .iter()
        .rposition(|byte| *byte == b'/')
        .map_or(0, |position| position + 1);
    head[directory_start..].eq_ignore_ascii_case(METALAYER)
}

/// The archive's first entry must be the marker, holding what A2 states.
fn marker(inventory: &ArchiveInventory<'_>) -> Result<(), RepackError> {
    let first = inventory
        .entries()
        .first()
        .ok_or_else(|| refuse(UnsupportedKind::Marker, None))?;
    if first.name_bytes() != MARKER_NAME.as_bytes() {
        return Err(refuse(UnsupportedKind::Marker, Some(0)));
    }
    let content = inventory.entry_bytes(0)?;
    if content != MARKER_CONTENT {
        return Err(refuse(UnsupportedKind::Marker, Some(0)));
    }
    Ok(())
}

/// Every entry after the document, each an attachment the layout numbers.
///
/// Entry `1 + n` must be `KRX/OCD/Payload/ID-<n>/<file>` with `<file>` one
/// component, so the numbering the writer would produce is the numbering the
/// package already has. Anything else is an entry the writer cannot place.
fn payloads(inventory: &ArchiveInventory<'_>) -> Result<Vec<Payload>, RepackError> {
    let mut payloads = Vec::with_capacity(inventory.len().saturating_sub(2));
    for (index, entry) in inventory.entries().iter().enumerate().skip(2) {
        let position =
            u32::try_from(index).map_err(|_| refuse(UnsupportedKind::ExtraEntry, None))?;
        let number = position - 1;
        let file_name = entry
            .name_text()
            .and_then(|name| payload_file_name(name, number))
            .ok_or_else(|| refuse(UnsupportedKind::ExtraEntry, Some(position)))?;
        payloads.push(Payload {
            entry: position,
            file_name,
            crc32: entry.crc32(),
        });
    }
    Ok(payloads)
}

/// The file name of `name`, when it is the payload entry of attachment
/// `number` and nothing else.
fn payload_file_name(name: &str, number: u32) -> Option<String> {
    let expected = format!("{PAYLOAD_PREFIX}{number}/");
    let file = name.strip_prefix(&expected)?;
    if file.is_empty() || file.contains('/') {
        return None;
    }
    Some(file.to_owned())
}
