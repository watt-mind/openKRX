//! Entry-name safety and collision rules.
//!
//! Names are checked as raw bytes, before any encoding decision. `docs/profile.md`
//! rule A8 fixes `/` as the only separator, and rule A21 leaves entry-name
//! encoding and case rules unresolved, so a case-folded collision is reported as
//! an ambiguity rather than resolved in either direction.

use crate::error::{AmbiguityKind, ArchiveError, UnsafeNameKind};

/// Reject a name shape that must never reach a filesystem layer.
///
/// `has_data` is true when the entry declares a non-zero uncompressed size; a
/// name ending in `/` then contradicts itself and is rejected.
pub(crate) fn check_name(name: &[u8], entry: u32, has_data: bool) -> Result<(), ArchiveError> {
    let unsafe_name = |kind| Err(ArchiveError::UnsafeName { kind, entry });
    if name.is_empty() {
        return unsafe_name(UnsafeNameKind::Empty);
    }
    if name.iter().any(|byte| *byte < 0x20 || *byte == 0x7f) {
        return unsafe_name(UnsafeNameKind::ControlByte);
    }
    if name.contains(&b'\\') {
        return unsafe_name(UnsafeNameKind::Backslash);
    }
    if name[0] == b'/' {
        return unsafe_name(UnsafeNameKind::AbsolutePath);
    }
    if has_drive_prefix(name) {
        return unsafe_name(UnsafeNameKind::DrivePrefix);
    }
    if name.split(|byte| *byte == b'/').any(|part| part == b"..") {
        return unsafe_name(UnsafeNameKind::ParentComponent);
    }
    if name.last() == Some(&b'/') && has_data {
        return unsafe_name(UnsafeNameKind::DirectoryWithData);
    }
    Ok(())
}

/// Detect a Windows drive prefix such as `c:` or `C:/x`.
///
/// Only position zero matters: a name starting with `/` has already been
/// rejected as absolute, which covers the `//c:/x` UNC-looking shapes too.
fn has_drive_prefix(name: &[u8]) -> bool {
    matches!(name, [letter, b':', ..] if letter.is_ascii_alphabetic())
}

/// Fold a name for collision detection.
///
/// Valid UTF-8 folds with the Unicode simple lowercase mapping; other bytes fold
/// with ASCII case only, because the intended code page is unresolved (A21).
pub(crate) fn fold_name(name: &[u8]) -> Vec<u8> {
    match core::str::from_utf8(name) {
        Ok(text) => text.to_lowercase().into_bytes(),
        Err(_) => name.to_ascii_lowercase(),
    }
}

/// Reject byte-identical and case-folded duplicate names.
///
/// `seen` accumulates `(raw, folded)` pairs in central-directory order.
pub(crate) fn check_collision(
    seen: &[(&[u8], Vec<u8>)],
    name: &[u8],
    folded: &[u8],
    entry: u32,
) -> Result<(), ArchiveError> {
    for (previous_raw, previous_folded) in seen {
        if *previous_raw == name {
            return Err(ArchiveError::Ambiguous {
                kind: AmbiguityKind::DuplicateName,
                entry: Some(entry),
            });
        }
        if previous_folded.as_slice() == folded {
            return Err(ArchiveError::Ambiguous {
                kind: AmbiguityKind::CaseFoldedDuplicateName,
                entry: Some(entry),
            });
        }
    }
    Ok(())
}
