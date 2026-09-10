//! Exit-status categories and the one place a failure is classified.
//!
//! Nine categories exist, they are published in
//! `docs/architecture.md#exit-statuses`, and they are stable: a consumer may
//! branch on the number. [`Category::of_code`] is the single classifier, and
//! [`Category::status`] the single number source, so a status can never be
//! invented at a call site.
//!
//! The core error enums are `#[non_exhaustive]`, so a downstream `match` on
//! their variants cannot be exhaustive and a new code cannot be made to fail
//! compilation here. Classification therefore keys on the code's own category
//! segment, which is part of the published contract, and
//! [`Category::of_code`] returns `None` for a segment it does not know. A test
//! reads every code out of `docs/codes.md` — the catalogue `check-codes.py`
//! forces to stay complete — and fails when one of them classifies to `None`.
//! It fixes no head list of its own: it takes every backticked dotted token as
//! a code and asserts that the heads it saw are exactly the ones the `HEADS`
//! line of `scripts/check-codes.py` names, so a new head cannot be added to
//! one side alone.
//!
//! This module holds the codes and the classifier; [`Failure`], the value a
//! refused run is reported as, and the sentence each code explains itself
//! with, are in [`failure`].

mod failure;

pub use failure::Failure;

/// Refusing to read more than this many bytes from the input.
///
/// One byte past the archive ceiling, so that an image exactly at the ceiling
/// is still read and an image past it is refused without buffering it.
pub const INPUT_CAP_BYTES: u64 = openkrx_core::Limits::DEFAULT.max_archive_bytes + 1;

/// The input could not be read at all.
pub const INPUT_UNREADABLE: &str = "input.unreadable";
/// The input is larger than [`INPUT_CAP_BYTES`], refused before parsing.
pub const INPUT_OVER_LIMIT: &str = "input.over_limit.archive_bytes";

/// The destination does not exist. `extract` never creates it.
pub const OUTPUT_DESTINATION_MISSING: &str = "output.destination_missing";
/// The destination exists but is not a directory.
pub const OUTPUT_DESTINATION_NOT_A_DIRECTORY: &str = "output.destination_not_a_directory";
/// The destination is a symbolic link or a reparse point.
pub const OUTPUT_DESTINATION_SYMLINK: &str = "output.destination_symlink";
/// The destination already holds an interrupted run's marker file.
pub const OUTPUT_PARTIAL_MARKER_PRESENT: &str = "output.partial_marker_present";
/// A path this run would create already exists, in any form.
pub const OUTPUT_EXISTS: &str = "output.exists";
/// An existing ancestor inside the destination is a link or a reparse point.
pub const OUTPUT_SYMLINK_IN_PATH: &str = "output.symlink_in_path";
/// An existing ancestor inside the destination is not a directory.
pub const OUTPUT_NOT_A_DIRECTORY: &str = "output.not_a_directory";
/// A create, write or remove failed. The underlying reason is never reported.
pub const OUTPUT_IO: &str = "output.io";

/// The manifest is not one JSON object.
pub const MANIFEST_SYNTAX: &str = "manifest.invalid.syntax";
/// The manifest declares a `schema_version` this build does not implement.
pub const MANIFEST_SCHEMA_VERSION: &str = "manifest.invalid.schema_version";
/// The manifest carries a key the schema does not define.
pub const MANIFEST_UNKNOWN_FIELD: &str = "manifest.invalid.unknown_field";
/// A required manifest field is absent.
pub const MANIFEST_MISSING_FIELD: &str = "manifest.invalid.missing_field";
/// A manifest field carries a value of the wrong JSON type.
pub const MANIFEST_TYPE: &str = "manifest.invalid.type";
/// A manifest field carries a token outside the fixed set the schema allows.
pub const MANIFEST_ENUMERATION: &str = "manifest.invalid.enumeration";
/// `timestamp` is not a time the MS-DOS fields of a ZIP record can express.
pub const MANIFEST_TIMESTAMP: &str = "manifest.invalid.timestamp";

/// A package `create` wrote did not read back cleanly. A defect in openKRX.
pub const CREATE_SELF_CHECK_FAILED: &str = "create.internal.self_check_failed";
/// A package `repack` wrote did not read back cleanly. A defect in openKRX.
pub const REPACK_SELF_CHECK_FAILED: &str = "repack.internal.self_check_failed";

/// What kind of outcome a run had, and therefore which status it exits with.
///
/// This enum is deliberately not `#[non_exhaustive]`: adding a category must
/// break every `match` in this crate, because each one is a contract decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    /// The command produced its report; `validate-structure` found no failure
    /// and nothing undecided.
    Success,
    /// The arguments were rejected. Produced by the argument parser only.
    Usage,
    /// A structural check failed.
    Inconsistent,
    /// No structural check failed, but at least one could not be decided.
    Unresolved,
    /// The input could not be read, or exceeded the input cap.
    Input,
    /// The package is malformed, truncated or ambiguous.
    Package,
    /// The package uses a feature this reader does not implement.
    Unsupported,
    /// A documented resource limit was exceeded.
    Limit,
    /// The destination could not be used, or a write failed. The two
    /// commands that write, `extract` and `create`, alone.
    Output,
}

impl Category {
    /// The process exit status for this category.
    #[must_use]
    pub const fn status(self) -> i32 {
        match self {
            Self::Success => 0,
            Self::Usage => 2,
            Self::Inconsistent => 3,
            Self::Unresolved => 4,
            Self::Input => 5,
            Self::Package => 6,
            Self::Unsupported => 7,
            Self::Limit => 8,
            Self::Output => 9,
        }
    }

    /// One content-free sentence saying what this category means.
    ///
    /// A stable code alone reads as a log line, and the reader of a package is
    /// not the person who chose the code. The sentence names the kind of
    /// problem and what could be done about it, using no part of the input:
    /// no path, no entry name, no declared value.
    #[must_use]
    pub const fn explanation(self) -> &'static str {
        match self {
            Self::Success => "the command produced its report",
            Self::Usage => "the arguments were rejected",
            Self::Inconsistent => "a structural check did not hold",
            Self::Unresolved => "a rule could not be decided from the sources",
            Self::Input => {
                "the input could not be read: check that the file exists, is \
readable, is a file rather than a directory, and is not larger than the 64 MiB \
input cap"
            }
            Self::Package => {
                "the package contradicts itself or is incomplete: it may have \
been truncated in transit, so obtaining it again is worth trying"
            }
            Self::Unsupported => {
                "the package uses a ZIP or XML feature openkrx does not \
implement; it is not damaged, and another reader may open it"
            }
            Self::Limit => {
                "the package is larger or more complex than a documented \
limit allows; the limits are not configurable from the command line"
            }
            Self::Output => {
                "the destination could not be written: it must already exist \
as a directory that is not a link, must not hold a file this package would \
have to overwrite, and must be writable"
            }
        }
    }

    /// The stable machine-readable name reported as `error.category`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Usage => "usage",
            Self::Inconsistent => "structure_inconsistent",
            Self::Unresolved => "structure_unresolved",
            Self::Input => "input",
            Self::Package => "package",
            Self::Unsupported => "unsupported",
            Self::Limit => "limit",
            Self::Output => "output",
        }
    }

    /// Classify a stable dotted code by its head and its category segment.
    ///
    /// `input.*` is classified before the general `*.over_limit.*` rule, so
    /// that an input larger than the cap is an input problem rather than a
    /// package that exceeded a parsing limit: nothing was parsed at all.
    /// `extract.*` follows the same segment rule as `archive.*`: a refused
    /// entry kind is an unsupported feature, an ambiguous or unsafe
    /// destination is a package problem, and a ceiling is a limit. `create.*`
    /// follows it too: a ceiling the output would exceed is a limit, and a
    /// request describing a package that contradicts itself, a name the
    /// writer refuses, or a package that did not read back cleanly, is a
    /// package problem. `manifest.invalid.*` joins them: a manifest describes
    /// a package that cannot be written, which is the same reading before
    /// anything exists to read. Every
    /// `output.*` code is [`Category::Output`], because each one is a fact
    /// about the destination rather than about the package.
    /// Returns `None` for a code shape this build does not know, which the
    /// catalogue test forbids.
    #[must_use]
    pub fn of_code(code: &str) -> Option<Self> {
        let mut segments = code.split('.');
        let head = segments.next()?;
        let kind = segments.next()?;
        Some(match (head, kind) {
            ("input", "unreadable" | "over_limit") => Self::Input,
            ("archive" | "create" | "extract" | "metadata", "over_limit") => Self::Limit,
            ("archive" | "extract" | "metadata", "unsupported") => Self::Unsupported,
            ("archive" | "extract" | "metadata", "truncated" | "malformed" | "ambiguous") => {
                Self::Package
            }
            ("archive", "unsafe_name" | "no_such_entry") => Self::Package,
            ("create", "invalid" | "unsafe_name" | "internal") => Self::Package,
            ("repack", "unsupported") => Self::Unsupported,
            ("repack", "invalid" | "internal") => Self::Package,
            ("manifest", "invalid") => Self::Package,
            ("extract", "unsafe_path") => Self::Package,
            ("metadata", "missing" | "reference" | "count_mismatch") => Self::Inconsistent,
            (
                "output",
                "destination_missing"
                | "destination_not_a_directory"
                | "destination_symlink"
                | "partial_marker_present"
                | "exists"
                | "symlink_in_path"
                | "not_a_directory"
                | "io",
            ) => Self::Output,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests;
