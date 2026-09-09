//! Resolving declared attachment references against real entry names.
//!
//! Rule M10 shows that a reference is `ELHELYEZKEDES` joined to `FAJL_NEV`, and
//! rule M6 gives `KRX/OCD/Payload` as the location default. Rule M14 records
//! that no source says whether that path is archive-root-relative, and rule A19
//! records that the archive root itself is unsettled. So a reference resolves
//! byte-exactly, or it resolves only after swapping one plausible root prefix
//! for another — which is reported as unresolved M14, never as a pass and never
//! as a failure.

use crate::archive::ArchiveInventory;
use crate::metadata::{AttachmentReference, Metadata};

use super::report::{AttachmentResolution, ReferenceResolution};

/// Root prefixes the primary sources disagree about (A19).
const KNOWN_PREFIXES: [&[u8]; 4] = [b"KRX/OCD/", b"OCD/", b"KRX/", b""];

/// Resolve every declared attachment against the archive's entry names.
pub(crate) fn resolve(
    inventory: &ArchiveInventory<'_>,
    metadata: &Metadata,
    root_prefix: &[u8],
) -> Vec<AttachmentResolution> {
    metadata
        .attachments()
        .map(|attachment| one(inventory, attachment, root_prefix))
        .collect()
}

/// Resolve one reference and report both declared and observed sizes.
fn one(
    inventory: &ArchiveInventory<'_>,
    attachment: &AttachmentReference,
    root_prefix: &[u8],
) -> AttachmentResolution {
    let declared_path = attachment.declared_path();
    let resolution = resolve_path(inventory, declared_path.as_bytes(), root_prefix);
    let observed_size = match resolution {
        ReferenceResolution::Resolved { entry } | ReferenceResolution::PrefixVariant { entry } => {
            super::report::observed_size(inventory.entries().get(entry as usize))
        }
        ReferenceResolution::Missing => None,
    };
    AttachmentResolution {
        number: attachment.number,
        declared_path,
        resolution,
        declared_size_text: attachment.size_text.clone(),
        declared_size_value: attachment.size_value,
        observed_size,
    }
}

/// Match `path` byte-exactly first, then against plausible prefix variants.
fn resolve_path(
    inventory: &ArchiveInventory<'_>,
    path: &[u8],
    root_prefix: &[u8],
) -> ReferenceResolution {
    if let Some(entry) = index_of(inventory, path) {
        return ReferenceResolution::Resolved { entry };
    }
    let tail = strip_known_prefix(path);
    for prefix in prefixes(root_prefix) {
        let mut candidate = Vec::with_capacity(prefix.len() + tail.len());
        candidate.extend_from_slice(prefix);
        candidate.extend_from_slice(tail);
        if candidate == path {
            continue;
        }
        if let Some(entry) = index_of(inventory, &candidate) {
            return ReferenceResolution::PrefixVariant { entry };
        }
    }
    ReferenceResolution::Missing
}

/// The prefixes worth trying: the one actually observed, then the known ones.
fn prefixes(root_prefix: &[u8]) -> impl Iterator<Item = &[u8]> {
    core::iter::once(root_prefix).chain(
        KNOWN_PREFIXES
            .into_iter()
            .filter(move |known| *known != root_prefix),
    )
}

/// Remove the longest known root prefix `path` starts with.
fn strip_known_prefix(path: &[u8]) -> &[u8] {
    KNOWN_PREFIXES
        .into_iter()
        .filter(|prefix| !prefix.is_empty() && path.starts_with(prefix))
        .max_by_key(|prefix| prefix.len())
        .map_or(path, |prefix| &path[prefix.len()..])
}

/// Central-directory index of the entry named `name`, byte-exactly.
fn index_of(inventory: &ArchiveInventory<'_>, name: &[u8]) -> Option<u32> {
    inventory
        .entries()
        .iter()
        .position(|entry| entry.name_bytes() == name)
        .and_then(|position| u32::try_from(position).ok())
}

/// Whether two references collide: same number, or same declared path (A6).
///
/// Rule A6 permits two attachments to share a file name in different payload
/// subdirectories, so only the joined path counts as a collision.
pub(crate) fn has_duplicate(attachments: &[AttachmentResolution]) -> bool {
    attachments.iter().enumerate().any(|(index, attachment)| {
        attachments[..index].iter().any(|earlier| {
            earlier.number == attachment.number || earlier.declared_path == attachment.declared_path
        })
    })
}

/// Whether any dispatch's `MELLEKLETEK_SZAMA` disagrees with its list (M7).
///
/// Returns `None` when no dispatch declared a count, so there is nothing to
/// compare and the check does not apply.
pub(crate) fn count_agrees(metadata: &Metadata) -> Option<bool> {
    let mut compared = false;
    let mut agrees = true;
    for dispatch in &metadata.dispatches {
        let Some(declared) = dispatch.declared_attachment_count else {
            continue;
        };
        compared = true;
        agrees &= i64::try_from(dispatch.attachments.len()).is_ok_and(|listed| listed == declared);
    }
    compared.then_some(agrees)
}
