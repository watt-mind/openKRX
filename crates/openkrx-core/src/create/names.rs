//! Entry names checked before they are written, against the reader's rules.
//!
//! Two rule sets already exist in this crate, and both apply here. The archive
//! layer's [`crate::archive::names::check_name`] decides what a name may be at
//! all; the extraction planner's `check_component` decides what a component may
//! be on any of the three target platforms. A writer that satisfied only the
//! first could emit a package its own `extract` command would refuse, which is
//! why both run over every name this module approves.
//!
//! The classes are re-reported under `create.unsafe_name.*` rather than under
//! the reading codes: the failure is in a request, not in a package, and the
//! caller needs to know which of the two it is. The name itself is never part
//! of a diagnostic.

use crate::archive::names::check_name;
use crate::error::{ArchiveError, UnsafeNameKind as ArchiveUnsafeNameKind};
use crate::extract::UnsafeComponentKind;
use crate::extract::paths::{check_component, components};

use super::error::{CreateError, UnsafeNameKind};

/// Check an attachment file name: one component, no separator of any kind.
///
/// `index` is the attachment's position in the request, which is what a
/// diagnostic reports; nothing else about the name is reported at all.
///
/// # Errors
///
/// `create.unsafe_name.empty` for an empty name, `create.unsafe_name.separator`
/// for a name holding `/` or `\`, and the class the component itself violates
/// otherwise.
pub(crate) fn check_file_name(file_name: &str, index: u32) -> Result<(), CreateError> {
    let refuse = |kind| {
        Err(CreateError::UnsafeName {
            kind,
            index: Some(index),
        })
    };
    if file_name.is_empty() {
        return refuse(UnsafeNameKind::Empty);
    }
    if file_name.contains('/') || file_name.contains('\\') {
        return refuse(UnsafeNameKind::Separator);
    }
    Ok(())
}

/// Check a whole entry name against both readers' rules.
///
/// `index` is present when the name belongs to an attachment, and absent for
/// the two fixed names, which are checked on the same path so that the layout
/// constants are held to the rules rather than trusted.
///
/// # Errors
///
/// A `create.unsafe_name.*` code naming the first class the name violates.
pub(crate) fn check_entry_name(name: &str, index: Option<u32>) -> Result<(), CreateError> {
    check_name(name.as_bytes(), index.unwrap_or_default(), true).map_err(|error| {
        CreateError::UnsafeName {
            kind: from_archive(&error),
            index,
        }
    })?;
    for component in components(name) {
        if let Err(kind) = check_component(component) {
            return Err(CreateError::UnsafeName {
                kind: from_component(kind),
                index,
            });
        }
    }
    Ok(())
}

/// Map an archive name class onto the creation class that reports it.
///
/// Every variant is mapped rather than defaulted, so that a class added to the
/// archive layer has to be considered here too — inside this crate the enums
/// are matched exhaustively, and a new variant stops the build until it is
/// classified. `Empty` stands in for a non-name error, which `check_name`
/// cannot produce: it returns [`ArchiveError::UnsafeName`] or nothing.
fn from_archive(error: &ArchiveError) -> UnsafeNameKind {
    let ArchiveError::UnsafeName { kind, .. } = error else {
        return UnsafeNameKind::Empty;
    };
    match kind {
        ArchiveUnsafeNameKind::Empty | ArchiveUnsafeNameKind::DirectoryWithData => {
            UnsafeNameKind::Empty
        }
        ArchiveUnsafeNameKind::ControlByte => UnsafeNameKind::ControlCharacter,
        ArchiveUnsafeNameKind::Backslash | ArchiveUnsafeNameKind::AbsolutePath => {
            UnsafeNameKind::Separator
        }
        ArchiveUnsafeNameKind::DrivePrefix => UnsafeNameKind::ReservedColon,
        ArchiveUnsafeNameKind::ParentComponent => UnsafeNameKind::ParentComponent,
    }
}

/// Map an extraction component class onto the creation class that reports it.
const fn from_component(kind: UnsafeComponentKind) -> UnsafeNameKind {
    match kind {
        UnsafeComponentKind::CurrentComponent => UnsafeNameKind::CurrentComponent,
        UnsafeComponentKind::ParentComponent => UnsafeNameKind::ParentComponent,
        UnsafeComponentKind::ControlCharacter => UnsafeNameKind::ControlCharacter,
        UnsafeComponentKind::TrailingDot => UnsafeNameKind::TrailingDot,
        UnsafeComponentKind::TrailingSpace => UnsafeNameKind::TrailingSpace,
        UnsafeComponentKind::LeadingSpace => UnsafeNameKind::LeadingSpace,
        UnsafeComponentKind::ReservedDeviceName => UnsafeNameKind::ReservedDeviceName,
        UnsafeComponentKind::Colon => UnsafeNameKind::ReservedColon,
        UnsafeComponentKind::ReservedCharacter => UnsafeNameKind::ReservedCharacter,
        UnsafeComponentKind::Empty => UnsafeNameKind::Empty,
    }
}
