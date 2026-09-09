//! Exit-status categories and the one place a failure is classified.
//!
//! Eight categories exist, they are published in
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

use openkrx_core::{ArchiveError, ProfileError};

/// Refusing to read more than this many bytes from the input.
///
/// One byte past the archive ceiling, so that an image exactly at the ceiling
/// is still read and an image past it is refused without buffering it.
pub const INPUT_CAP_BYTES: u64 = openkrx_core::Limits::DEFAULT.max_archive_bytes + 1;

/// The input could not be read at all.
pub const INPUT_UNREADABLE: &str = "input.unreadable";
/// The input is larger than [`INPUT_CAP_BYTES`], refused before parsing.
pub const INPUT_OVER_LIMIT: &str = "input.over_limit.archive_bytes";

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
        }
    }

    /// Classify a stable dotted code by its head and its category segment.
    ///
    /// `input.*` is classified before the general `*.over_limit.*` rule, so
    /// that an input larger than the cap is an input problem rather than a
    /// package that exceeded a parsing limit: nothing was parsed at all.
    /// Returns `None` for a code shape this build does not know, which the
    /// catalogue test forbids.
    #[must_use]
    pub fn of_code(code: &str) -> Option<Self> {
        let mut segments = code.split('.');
        let head = segments.next()?;
        let kind = segments.next()?;
        Some(match (head, kind) {
            ("input", "unreadable" | "over_limit") => Self::Input,
            ("archive" | "metadata", "over_limit") => Self::Limit,
            ("archive" | "metadata", "unsupported") => Self::Unsupported,
            ("archive" | "metadata", "truncated" | "malformed" | "ambiguous") => Self::Package,
            ("archive", "unsafe_name" | "no_such_entry") => Self::Package,
            ("metadata", "missing" | "reference" | "count_mismatch") => Self::Inconsistent,
            _ => return None,
        })
    }
}

/// A run that ended before a report could be produced.
///
/// The fields are exactly what a diagnostic may carry: a stable code, its
/// category, an entry index and numbers. An input path, an entry name and a
/// metadata value are never reachable from here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Failure {
    /// The stable dotted code.
    pub code: &'static str,
    /// The category the code classifies to.
    pub category: Category,
    /// Central-directory index of the entry the failure concerns, if any.
    pub entry_index: Option<u32>,
    /// The configured ceiling, when the failure is a limit.
    pub limit: Option<u64>,
    /// The value that reached the ceiling, when it is known.
    pub observed: Option<u64>,
}

impl Failure {
    /// Classify `code`, treating an unknown shape as a package problem.
    fn new(code: &'static str) -> Self {
        Self {
            code,
            category: Category::of_code(code).unwrap_or(Category::Package),
            entry_index: None,
            limit: None,
            observed: None,
        }
    }

    /// The input could not be opened or read.
    #[must_use]
    pub fn unreadable() -> Self {
        Self::new(INPUT_UNREADABLE)
    }

    /// The input is larger than the cap; `observed` is the cap itself, because
    /// the reader stops there and never learns the real length.
    #[must_use]
    pub fn over_input_cap() -> Self {
        Self {
            limit: Some(openkrx_core::Limits::DEFAULT.max_archive_bytes),
            observed: Some(INPUT_CAP_BYTES),
            ..Self::new(INPUT_OVER_LIMIT)
        }
    }

    /// The message a human-mode diagnostic prints: code, numbers, entry index.
    #[must_use]
    pub fn message(&self) -> String {
        let mut text = self.code.to_owned();
        if let Some(limit) = self.limit {
            text.push_str(&format!(" (limit {limit}"));
            if let Some(observed) = self.observed {
                text.push_str(&format!(", observed {observed}"));
            }
            text.push(')');
        }
        if let Some(entry) = self.entry_index {
            text.push_str(&format!(" at entry {entry}"));
        }
        text
    }
}

impl From<ArchiveError> for Failure {
    fn from(error: ArchiveError) -> Self {
        let (limit, observed) = match error {
            ArchiveError::OverLimit {
                limit_value,
                observed,
                ..
            } => (Some(limit_value), observed),
            ArchiveError::Unsupported { value, .. } => (None, value),
            _ => (None, None),
        };
        Self {
            entry_index: error.entry_index(),
            limit,
            observed,
            ..Self::new(error.code())
        }
    }
}

impl From<ProfileError> for Failure {
    fn from(error: ProfileError) -> Self {
        match error {
            ProfileError::Archive(archive) => Self::from(archive),
            // The enum is `#[non_exhaustive]`; a future variant still carries
            // a stable code, which is all a diagnostic is allowed to report.
            _ => Self::new(error.code()),
        }
    }
}

#[cfg(test)]
mod tests {
    //! One test per exit-status category, and one that holds the classifier
    //! against the whole published catalogue.
    //!
    //! These are unit tests rather than subprocess tests because the mapping
    //! is a pure function of a code string: a subprocess can reach only the
    //! codes an archive can be built to produce, and the contract covers every
    //! code the crates define.

    use super::{Category, Failure};
    use openkrx_core::{ArchiveError, LimitKind, MalformedKind, Structure, UnsupportedKind};

    /// The catalogue `scripts/check-codes.py` forces to stay complete.
    const CATALOGUE: &str = include_str!("../../../docs/codes.md");

    #[test]
    fn success_is_zero() {
        assert_eq!(Category::Success.status(), 0);
        assert_eq!(Category::Success.as_str(), "success");
    }

    #[test]
    fn a_usage_error_is_two() {
        assert_eq!(Category::Usage.status(), 2);
    }

    #[test]
    fn a_failing_check_is_three() {
        assert_eq!(Category::Inconsistent.status(), 3);
        assert_eq!(
            Category::of_code("metadata.reference.missing_entry"),
            Some(Category::Inconsistent)
        );
        assert_eq!(
            Category::of_code("metadata.count_mismatch"),
            Some(Category::Inconsistent)
        );
        assert_eq!(
            Category::of_code("metadata.missing"),
            Some(Category::Inconsistent)
        );
    }

    #[test]
    fn an_undecided_rule_is_four() {
        assert_eq!(Category::Unresolved.status(), 4);
        assert_eq!(Category::Unresolved.as_str(), "structure_unresolved");
    }

    #[test]
    fn an_input_problem_is_five() {
        assert_eq!(Category::Input.status(), 5);
        assert_eq!(Failure::unreadable().category, Category::Input);
        let over = Failure::over_input_cap();
        assert_eq!(over.category, Category::Input);
        assert_eq!(over.code, "input.over_limit.archive_bytes");
        assert_eq!(over.limit, Some(64 * 1024 * 1024));
        assert_eq!(over.observed, Some(super::INPUT_CAP_BYTES));
    }

    #[test]
    fn a_malformed_truncated_or_ambiguous_package_is_six() {
        assert_eq!(Category::Package.status(), 6);
        for code in [
            "archive.malformed.eocd_missing",
            "archive.truncated.entry_data",
            "archive.ambiguous.duplicate_name",
            "archive.unsafe_name.empty",
            "archive.no_such_entry",
            "metadata.malformed.syntax",
        ] {
            assert_eq!(Category::of_code(code), Some(Category::Package), "{code}");
        }
        let failure = Failure::from(ArchiveError::Truncated {
            at: Structure::EntryData,
            entry: Some(3),
        });
        assert_eq!(failure.category, Category::Package);
        assert_eq!(failure.entry_index, Some(3));
        assert_eq!(failure.message(), "archive.truncated.entry_data at entry 3");
    }

    #[test]
    fn an_unsupported_feature_is_seven() {
        assert_eq!(Category::Unsupported.status(), 7);
        let failure = Failure::from(ArchiveError::Unsupported {
            kind: UnsupportedKind::Method,
            value: Some(12),
            entry: Some(1),
        });
        assert_eq!(failure.category, Category::Unsupported);
        assert_eq!(failure.observed, Some(12));
        assert_eq!(failure.code, "archive.unsupported.method");
    }

    #[test]
    fn a_resource_limit_is_eight() {
        assert_eq!(Category::Limit.status(), 8);
        let failure = Failure::from(ArchiveError::OverLimit {
            limit: LimitKind::Entries,
            limit_value: 256,
            observed: Some(300),
            entry: None,
        });
        assert_eq!(failure.category, Category::Limit);
        assert_eq!(
            failure.message(),
            "archive.over_limit.entries (limit 256, observed 300)"
        );
    }

    #[test]
    fn every_status_is_distinct_and_only_success_is_zero() {
        let categories = [
            Category::Success,
            Category::Usage,
            Category::Inconsistent,
            Category::Unresolved,
            Category::Input,
            Category::Package,
            Category::Unsupported,
            Category::Limit,
        ];
        let mut statuses: Vec<i32> = categories.iter().map(|kind| kind.status()).collect();
        statuses.sort_unstable();
        assert_eq!(statuses, vec![0, 2, 3, 4, 5, 6, 7, 8]);
        for category in categories {
            assert_eq!(category.status() == 0, category == Category::Success);
            assert!(!category.as_str().is_empty());
        }
    }

    /// Whether a backticked span of the catalogue is a code rather than prose.
    ///
    /// The document also writes `archive.*` and `metadata.` in running text,
    /// so a span counts only when it is a full dotted code: two or more
    /// segments of lower-case letters, digits and underscores.
    fn code_shaped(piece: &str) -> bool {
        let mut segments = piece.split('.');
        if !matches!(segments.next(), Some("archive" | "metadata")) {
            return false;
        }
        let rest: Vec<&str> = segments.collect();
        !rest.is_empty()
            && rest.iter().all(|segment| {
                !segment.is_empty()
                    && segment.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
                    })
            })
    }

    #[test]
    fn every_catalogued_code_classifies_to_a_category() {
        let mut seen = 0_usize;
        for line in CATALOGUE.lines() {
            for piece in line.split('`').skip(1).step_by(2) {
                if !code_shaped(piece) {
                    continue;
                }
                seen += 1;
                assert!(
                    Category::of_code(piece).is_some(),
                    "docs/codes.md lists {piece}, which this build cannot \
classify; add its category segment to Category::of_code"
                );
            }
        }
        assert!(seen > 60, "the catalogue was read, {seen} codes found");
    }

    #[test]
    fn an_unknown_code_shape_is_never_a_success() {
        assert_eq!(Category::of_code("nonsense"), None);
        // Built rather than written, so that the catalogue checker does not
        // read this deliberately unknown shape as a code the crate defines.
        assert_eq!(
            Category::of_code(&format!("archive.{}.x", "brand_new_category")),
            None
        );
        assert_eq!(
            Failure::from(ArchiveError::Malformed {
                kind: MalformedKind::CrcMismatch,
                entry: None,
            })
            .category,
            Category::Package
        );
    }
}
