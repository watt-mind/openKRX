//! Destination path components, and the shapes that must never become one.
//!
//! An entry name is split on `/`, the only separator rule A8 permits; the
//! archive layer has already refused a backslash, so no other separator can
//! reach this module. Each component is then checked against the union of the
//! rules the three target platforms need, not against the rules of the platform
//! the planner happens to run on: a plan is a portable value, and a package that
//! only extracts safely on Linux would be a trap on Windows.
//!
//! Several classes here — `..`, a C0 control, an empty first component — are
//! already impossible in an accepted inventory. They are checked anyway, so this
//! module's output is safe on its own terms rather than because another module
//! is assumed to have run first.

use super::error::UnsafeComponentKind;

/// Characters no Windows filesystem accepts in a name.
///
/// `\` is not listed: the archive layer refuses a name holding one before it
/// can reach the planner, and `:` and `/` have codes of their own.
const RESERVED_CHARACTERS: [char; 6] = ['*', '?', '<', '>', '|', '"'];

/// Windows reserved device names, upper case, without an extension.
const RESERVED_DEVICE_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Split a file entry's name into destination components.
///
/// The name is used verbatim: nothing is stripped, prefixed or rewritten, so a
/// plan can be compared against the archive it came from.
pub(crate) fn components(name: &str) -> Vec<&str> {
    name.split('/').collect()
}

/// Check one component, returning the first class it violates.
///
/// The order of the checks is fixed so that a given component always produces
/// the same code: emptiness, the two relative names, control characters, the
/// two trailing characters that filesystems strip, a leading space, the
/// reserved device names, the colon, and finally the six characters Windows
/// refuses outright.
pub(crate) fn check_component(component: &str) -> Result<(), UnsafeComponentKind> {
    if component.is_empty() {
        return Err(UnsafeComponentKind::Empty);
    }
    if component == "." {
        return Err(UnsafeComponentKind::CurrentComponent);
    }
    if component == ".." {
        return Err(UnsafeComponentKind::ParentComponent);
    }
    if component.chars().any(char::is_control) {
        return Err(UnsafeComponentKind::ControlCharacter);
    }
    if component.ends_with('.') {
        return Err(UnsafeComponentKind::TrailingDot);
    }
    if component.ends_with(' ') {
        return Err(UnsafeComponentKind::TrailingSpace);
    }
    if component.starts_with(' ') {
        return Err(UnsafeComponentKind::LeadingSpace);
    }
    if is_reserved_device_name(component) {
        return Err(UnsafeComponentKind::ReservedDeviceName);
    }
    if component.contains(':') {
        return Err(UnsafeComponentKind::Colon);
    }
    if component.contains(RESERVED_CHARACTERS) {
        return Err(UnsafeComponentKind::ReservedCharacter);
    }
    Ok(())
}

/// Whether a component is a Windows reserved device name.
///
/// Windows resolves `CON`, `con.txt` and `CON.tar.gz` alike to the console
/// device, so the stem before the first `.` is what matters. The comparison is
/// ASCII case-insensitive, matching the platform rule rather than a Unicode
/// case fold.
fn is_reserved_device_name(component: &str) -> bool {
    let stem = component.split('.').next().unwrap_or(component);
    RESERVED_DEVICE_NAMES
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
}

#[cfg(test)]
mod tests {
    use super::{UnsafeComponentKind, check_component, components};

    #[track_caller]
    fn refuses(component: &str, kind: UnsafeComponentKind) {
        assert_eq!(check_component(component), Err(kind), "{component:?}");
    }

    #[test]
    fn a_name_splits_on_the_only_separator() {
        assert_eq!(
            components("Payload/ID-1/x.pdf"),
            ["Payload", "ID-1", "x.pdf"]
        );
        assert_eq!(components("mimetype"), ["mimetype"]);
    }

    #[test]
    fn every_unsafe_component_class_is_refused() {
        refuses("", UnsafeComponentKind::Empty);
        refuses(".", UnsafeComponentKind::CurrentComponent);
        refuses("..", UnsafeComponentKind::ParentComponent);
        refuses("a\u{0}b", UnsafeComponentKind::ControlCharacter);
        refuses("a\u{1}b", UnsafeComponentKind::ControlCharacter);
        refuses("a\u{7f}b", UnsafeComponentKind::ControlCharacter);
        refuses("a\u{85}b", UnsafeComponentKind::ControlCharacter);
        refuses("a.", UnsafeComponentKind::TrailingDot);
        refuses("a ", UnsafeComponentKind::TrailingSpace);
        refuses(" a", UnsafeComponentKind::LeadingSpace);
        refuses("CON", UnsafeComponentKind::ReservedDeviceName);
        refuses("con.txt", UnsafeComponentKind::ReservedDeviceName);
        refuses("LPT9.tar.gz", UnsafeComponentKind::ReservedDeviceName);
        refuses("a:b", UnsafeComponentKind::Colon);
        for character in ['*', '?', '<', '>', '|', '"'] {
            refuses(
                &format!("a{character}b"),
                UnsafeComponentKind::ReservedCharacter,
            );
        }
    }

    #[test]
    fn the_class_no_archive_can_carry_reports_its_stable_code() {
        // Every other class is asserted by code in `tests/extract_rejects.rs`,
        // over an archive that reaches the planner. A `..` component is refused
        // by the archive layer first, so its literal is pinned here.
        assert_eq!(
            check_component(".."),
            Err(UnsafeComponentKind::ParentComponent)
        );
        assert_eq!(
            UnsafeComponentKind::ParentComponent.code(),
            "extract.unsafe_path.parent_component"
        );
    }

    #[test]
    fn a_component_that_merely_looks_unusual_is_accepted() {
        for component in [
            "a.b",
            "CONSOLE",
            "COM0",
            "COM10",
            "NULL",
            ".hidden",
            "..hidden",
            "árvíztűrő.pdf",
            "a b",
            "ID-1",
        ] {
            assert_eq!(check_component(component), Ok(()), "{component:?}");
        }
    }
}
