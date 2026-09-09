//! Finding the metadata document and the format marker by observation.
//!
//! `docs/profile.md` rule A19 records three primary sources describing three
//! different directory layouts, and rule M12 records two spellings of the
//! metadata file name. Neither is settled, so nothing here assumes a layout:
//! the candidate set is defined by shape, and whatever prefix and casing the
//! archive actually used is reported as an observation.

use crate::archive::{ArchiveEntry, ArchiveInventory};

/// The directory the metadata document sits in (A4).
const METALAYER: &[u8] = b"Metalayer";
/// The metadata document's canonical file name (A4, M12).
const METADATA_FILE: &[u8] = b"KULDEMENY_META.xml";
/// The format marker entry's name (A2).
pub(crate) const MARKER_NAME: &[u8] = b"mimetype";
/// The format marker entry's content (A2).
pub(crate) const MARKER_CONTENT: &[u8] = b"application/OCD+ZIP";

/// The one metadata candidate an archive holds, and how it was spelled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Located {
    /// Central-directory index of the entry.
    pub(crate) index: u32,
    /// The entry name exactly as the archive spells it.
    pub(crate) name: Vec<u8>,
    /// Everything before `Metalayer/`, such as `KRX/OCD/` (A19).
    pub(crate) prefix: Vec<u8>,
    /// Whether the last two segments are spelled canonically (M12).
    pub(crate) canonical_name: bool,
}

/// Why an archive holds no single metadata candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocateError {
    /// No entry has the shape of a metadata document.
    Missing,
    /// More than one entry does, and nothing settles which one is meant.
    Ambiguous,
}

/// Find the single metadata candidate, by shape rather than by assumption.
///
/// A candidate is any entry whose last path segment equals
/// `KULDEMENY_META.xml` and whose parent segment equals `Metalayer`, both
/// compared ASCII-case-insensitively, because M12 leaves casing open and A21
/// leaves entry-name case rules open. Any number of leading segments is
/// allowed, because A19 leaves the nesting open.
pub(crate) fn locate(inventory: &ArchiveInventory<'_>) -> Result<Located, LocateError> {
    let mut found: Option<Located> = None;
    for (index, entry) in inventory.entries().iter().enumerate() {
        let Some(candidate) = candidate(entry.name_bytes(), index as u32) else {
            continue;
        };
        if found.is_some() {
            return Err(LocateError::Ambiguous);
        }
        found = Some(candidate);
    }
    found.ok_or(LocateError::Missing)
}

/// Describe `name` as a metadata candidate, when it has that shape.
fn candidate(name: &[u8], index: u32) -> Option<Located> {
    let file_start = name.iter().rposition(|byte| *byte == b'/')? + 1;
    let file = &name[file_start..];
    if !file.eq_ignore_ascii_case(METADATA_FILE) {
        return None;
    }
    let head = &name[..file_start - 1];
    let directory_start = head
        .iter()
        .rposition(|byte| *byte == b'/')
        .map_or(0, |position| position + 1);
    let directory = &head[directory_start..];
    if !directory.eq_ignore_ascii_case(METALAYER) {
        return None;
    }
    Some(Located {
        index,
        name: name.to_vec(),
        prefix: name[..directory_start].to_vec(),
        canonical_name: file == METADATA_FILE && directory == METALAYER,
    })
}

/// What an archive holds where rule A2 expects the format marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Marker {
    /// A first entry named `mimetype` holding `application/OCD+ZIP`.
    Matching,
    /// A first entry named `mimetype` holding something else.
    ContentMismatch,
    /// An entry named `mimetype` that is not the first entry.
    NotFirst,
    /// A marker entry under a directory prefix, which A19 leaves open.
    Prefixed,
    /// No entry whose last path segment is `mimetype`.
    Missing,
}

/// Observe the marker entry, without deciding what A19 leaves open.
///
/// Rule A20 — the marker's compression method, extra fields and byte-exactness
/// — is deliberately never asserted: no source states it, so this function
/// looks at the entry's decoded content and nothing else.
pub(crate) fn marker(inventory: &ArchiveInventory<'_>) -> Result<Marker, crate::ArchiveError> {
    let Some((index, entry)) = find_marker(inventory) else {
        return Ok(Marker::Missing);
    };
    if entry.name_bytes() != MARKER_NAME {
        return Ok(Marker::Prefixed);
    }
    if index != 0 {
        return Ok(Marker::NotFirst);
    }
    let content = inventory.entry_bytes(index)?;
    if content == MARKER_CONTENT {
        Ok(Marker::Matching)
    } else {
        Ok(Marker::ContentMismatch)
    }
}

/// The first entry whose last path segment is exactly `mimetype`.
fn find_marker<'a, 'b>(inventory: &'b ArchiveInventory<'a>) -> Option<(u32, &'b ArchiveEntry<'a>)> {
    inventory
        .entries()
        .iter()
        .enumerate()
        .find(|(_, entry)| last_segment(entry.name_bytes()) == MARKER_NAME)
        .map(|(index, entry)| (index as u32, entry))
}

/// The part of `name` after its last `/`.
fn last_segment(name: &[u8]) -> &[u8] {
    match name.iter().rposition(|byte| *byte == b'/') {
        Some(position) => &name[position + 1..],
        None => name,
    }
}
