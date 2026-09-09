//! Two outputs that cannot both exist, detected before anything is written.
//!
//! `SECURITY.md` requires Unicode and case collisions to be specified and
//! refused, never resolved. Renaming one of two colliding outputs would hand a
//! caller a file whose name is not the name the package declares; skipping one
//! would report success over a partial result. Both are refused instead.
//!
//! Two destination paths collide when they are equal after NFC normalisation
//! and case folding, which is what a case-insensitive normalising filesystem —
//! APFS, NTFS, and a case-insensitive ext4 directory — would see. A path also
//! conflicts when it is a directory prefix of another, because the same name
//! would have to be a file and a directory at once.

use std::collections::HashMap;

use unicode_normalization::UnicodeNormalization;

use super::error::{PlanAmbiguityKind, PlanError};

/// Fold one destination path into the key a colliding filesystem would compare.
///
/// The components are normalised to NFC and lower-cased individually, then
/// joined with `/`; a component can never hold `/`, so the joined form is
/// unambiguous. Lower-casing is `char::to_lowercase`, the Unicode simple
/// lowercase mapping, which approximates full case folding: it agrees with it
/// for the Latin, Greek and Cyrillic text these names can realistically hold,
/// and the residual risk is recorded in `SECURITY.md`.
fn fold(components: &[String]) -> String {
    let mut key = String::new();
    for component in components {
        if !key.is_empty() {
            key.push('/');
        }
        for character in component.nfc() {
            key.extend(character.to_lowercase());
        }
    }
    key
}

/// Refuse two outputs that a filesystem could not keep apart.
///
/// `paths` are the planned destination paths in inventory order. The error
/// names the later of the two entries: it is the one whose output could not be
/// created, and reporting it keeps the diagnostic stable whatever the earlier
/// entry was.
pub(crate) fn check(paths: &[(u32, Vec<String>)]) -> Result<(), PlanError> {
    let mut files: HashMap<String, u32> = HashMap::with_capacity(paths.len());
    for (entry, components) in paths {
        let key = fold(components);
        if let Some(previous) = files.insert(key, *entry) {
            return Err(PlanError::Ambiguous {
                kind: PlanAmbiguityKind::Collision,
                entry: (*entry).max(previous),
            });
        }
    }
    for (entry, components) in paths {
        let mut prefix = String::new();
        for component in &components[..components.len() - 1] {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            for character in component.nfc() {
                prefix.extend(character.to_lowercase());
            }
            if let Some(previous) = files.get(&prefix) {
                return Err(PlanError::Ambiguous {
                    kind: PlanAmbiguityKind::FileDirectoryConflict,
                    entry: (*entry).max(*previous),
                });
            }
        }
    }
    Ok(())
}

/// Every implicit parent directory of `paths`, deduplicated and sorted.
///
/// A directory marker in the archive contributes nothing: directories exist in
/// a plan only because a file sits beneath them, so an empty directory is never
/// materialised. The result is sorted component-wise, which places a parent
/// before every child and makes the order independent of inventory order.
pub(crate) fn directories(paths: &[(u32, Vec<String>)]) -> Vec<Vec<String>> {
    let mut all: Vec<Vec<String>> = Vec::new();
    for (_, components) in paths {
        for depth in 1..components.len() {
            all.push(components[..depth].to_vec());
        }
    }
    all.sort();
    all.dedup();
    all
}

#[cfg(test)]
mod tests {
    use super::{directories, fold};

    #[test]
    fn folding_agrees_on_normalisation_and_case() {
        let composed = vec!["Á".to_string(), "b".to_string()];
        let decomposed = vec!["A\u{301}".to_string(), "B".to_string()];
        assert_eq!(fold(&composed), fold(&decomposed));
        assert_ne!(fold(&composed), fold(&[String::from("a"), "b".to_string()]));
    }

    #[test]
    fn implicit_parents_are_listed_once_each_in_sorted_order() {
        let paths = vec![
            (0, vec!["b".to_string(), "x".to_string()]),
            (1, vec!["a".to_string(), "c".to_string(), "y".to_string()]),
            (2, vec!["a".to_string(), "c".to_string(), "z".to_string()]),
            (3, vec!["top".to_string()]),
        ];
        assert_eq!(
            directories(&paths),
            vec![
                vec!["a".to_string()],
                vec!["a".to_string(), "c".to_string()],
                vec!["b".to_string()],
            ]
        );
    }
}
