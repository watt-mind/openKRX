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
mod tests {
    //! One test per exit-status category, and one that holds the classifier
    //! against the whole published catalogue.
    //!
    //! These are unit tests rather than subprocess tests because the mapping
    //! is a pure function of a code string: a subprocess can reach only the
    //! codes an archive can be built to produce, and the contract covers every
    //! code the crates define.

    use super::{Category, Failure};
    use openkrx_core::extract::{ExtractLimitKind, UnsupportedEntryKind};
    use openkrx_core::{
        ArchiveError, LimitKind, MalformedKind, PlanError, Structure, UnsupportedKind,
    };

    /// Every category, in status order. Adding one must be added here too.
    const EVERY_CATEGORY: [Category; 9] = [
        Category::Success,
        Category::Usage,
        Category::Inconsistent,
        Category::Unresolved,
        Category::Input,
        Category::Package,
        Category::Unsupported,
        Category::Limit,
        Category::Output,
    ];

    /// The catalogue `scripts/check-codes.py` forces to stay complete.
    const CATALOGUE: &str = include_str!("../../../../docs/codes.md");

    /// The checker that owns the one head list, read for that list alone.
    const CHECKER: &str = include_str!("../../../../scripts/check-codes.py");

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
    fn a_destination_or_write_problem_is_nine() {
        assert_eq!(Category::Output.status(), 9);
        assert_eq!(Category::Output.as_str(), "output");
        for code in [
            super::OUTPUT_DESTINATION_MISSING,
            super::OUTPUT_DESTINATION_NOT_A_DIRECTORY,
            super::OUTPUT_DESTINATION_SYMLINK,
            super::OUTPUT_PARTIAL_MARKER_PRESENT,
            super::OUTPUT_EXISTS,
            super::OUTPUT_SYMLINK_IN_PATH,
            super::OUTPUT_NOT_A_DIRECTORY,
            super::OUTPUT_IO,
        ] {
            assert_eq!(Category::of_code(code), Some(Category::Output), "{code}");
            assert_eq!(Failure::output(code).category, Category::Output);
        }
        let scoped = Failure::output_at(super::OUTPUT_EXISTS, 7);
        assert_eq!(scoped.message(), "output.exists at entry 7");
        assert!(scoped.line().ends_with("(exit 9)"));
        assert!(scoped.line().contains("nothing is ever overwritten"));
        // Each output code says what happened rather than sharing one
        // sentence that describes all nine conditions at once.
        let missing = Failure::output(super::OUTPUT_DESTINATION_MISSING).line();
        assert!(missing.contains("does not exist"));
        assert!(!missing.contains("overwritten"), "{missing}");
    }

    #[test]
    fn a_manifest_refusal_names_its_field_and_never_a_value() {
        // Every manifest code is a package problem, and the diagnostic says
        // which field it concerns using the schema's own path — the manifest's
        // values, and the spelling of an unknown key, are not reachable here.
        for code in [
            super::MANIFEST_SYNTAX,
            super::MANIFEST_SCHEMA_VERSION,
            super::MANIFEST_UNKNOWN_FIELD,
            super::MANIFEST_MISSING_FIELD,
            super::MANIFEST_TYPE,
            super::MANIFEST_ENUMERATION,
            super::MANIFEST_TIMESTAMP,
        ] {
            assert_eq!(Category::of_code(code), Some(Category::Package), "{code}");
            let failure = Failure::manifest(code, "/metadata/source_system");
            assert_eq!(failure.category.status(), 6);
            assert!(
                failure
                    .message()
                    .ends_with("at field /metadata/source_system")
            );
            assert!(failure.line().ends_with("(exit 6)"));
            // Each code carries its own sentence rather than the category's.
            assert!(!failure.line().contains("truncated in transit"), "{code}");
        }
        let scoped = Failure::manifest_at(super::MANIFEST_TYPE, "/attachments/path", 2);
        assert_eq!(scoped.attachment_index, Some(2));
        assert_eq!(
            scoped.message(),
            "manifest.invalid.type at field /attachments/path (attachment 2)"
        );
        assert_eq!(scoped.entry_index, None, "no archive exists yet");
    }

    #[test]
    fn the_self_check_code_is_a_package_problem() {
        // A package openKRX wrote that its own reader refuses is a defect in
        // openKRX; the status must still be a failure, and the sentence must
        // say so rather than blame the caller's manifest.
        let failure = Failure::self_check_failed();
        assert_eq!(failure.code, "create.internal.self_check_failed");
        assert_eq!(failure.category, Category::Package);
        assert_eq!(failure.category.status(), 6);
        assert!(failure.line().contains("defect in openkrx"));
        assert!(failure.line().contains("nothing was left behind"));
    }

    #[test]
    fn a_creation_refusal_points_at_the_attachment_rather_than_an_entry() {
        use openkrx_core::create::{CreateError, CreateLimitKind, InvalidKind, UnsafeNameKind};

        let invalid = Failure::from(CreateError::Invalid {
            kind: InvalidKind::ReferenceMismatch,
            index: Some(1),
        });
        assert_eq!(invalid.code, "create.invalid.reference_mismatch");
        assert_eq!(invalid.category, Category::Package);
        assert_eq!(invalid.attachment_index, Some(1));
        assert_eq!(invalid.entry_index, None);

        let unsafe_name = Failure::from(CreateError::UnsafeName {
            kind: UnsafeNameKind::Separator,
            index: Some(0),
        });
        assert_eq!(unsafe_name.code, "create.unsafe_name.separator");
        assert_eq!(unsafe_name.category, Category::Package);

        let over = Failure::from(CreateError::OverLimit {
            limit: CreateLimitKind::Entries,
            limit_value: 256,
            observed: Some(300),
            index: None,
        });
        assert_eq!(over.category, Category::Limit);
        assert_eq!(
            over.message(),
            "create.over_limit.entries (limit 256, observed 300)"
        );
    }

    #[test]
    fn a_planning_refusal_keeps_the_category_its_segment_names() {
        let unsupported = Failure::from(PlanError::Unsupported {
            kind: UnsupportedEntryKind::Link,
            entry: 2,
        });
        assert_eq!(unsupported.category, Category::Unsupported);
        assert_eq!(unsupported.code, "extract.unsupported.link");
        assert_eq!(unsupported.entry_index, Some(2));

        let over = Failure::from(PlanError::OverLimit {
            limit: ExtractLimitKind::Files,
            limit_value: 256,
            observed: Some(257),
            entry: Some(256),
        });
        assert_eq!(over.category, Category::Limit);
        assert_eq!(
            over.message(),
            "extract.over_limit.files (limit 256, observed 257) at entry 256"
        );
        for code in [
            "extract.unsafe_path.parent_component",
            "extract.ambiguous.collision",
        ] {
            assert_eq!(Category::of_code(code), Some(Category::Package), "{code}");
        }
    }

    #[test]
    fn a_diagnostic_line_explains_its_category_without_naming_the_input() {
        let line = Failure::over_input_cap().line();
        assert!(line.starts_with("openkrx: input.over_limit.archive_bytes"));
        assert!(line.contains("could not be read"));
        assert!(line.ends_with("(exit 5)"));
        for category in EVERY_CATEGORY {
            assert!(!category.explanation().is_empty());
        }
    }

    #[test]
    fn every_status_is_distinct_and_only_success_is_zero() {
        let categories = EVERY_CATEGORY;
        let mut statuses: Vec<i32> = categories.iter().map(|kind| kind.status()).collect();
        statuses.sort_unstable();
        assert_eq!(statuses, vec![0, 2, 3, 4, 5, 6, 7, 8, 9]);
        for category in categories {
            assert_eq!(category.status() == 0, category == Category::Success);
            assert!(!category.as_str().is_empty());
        }
    }

    /// Whether a backticked span of the catalogue is a code rather than prose.
    ///
    /// No head is fixed here: a span counts when it is a full dotted code —
    /// a lower-case head followed by one or more segments of lower-case
    /// letters, digits and underscores — so a new head is picked up the moment
    /// the catalogue documents it. The document also writes `archive.*` and
    /// `metadata.` in running text, and paths such as `check-codes.py`, none of
    /// which match that shape.
    fn code_shaped(piece: &str) -> bool {
        let mut segments = piece.split('.');
        let head = segments.next().unwrap_or_default();
        if head.is_empty() || !head.bytes().all(|byte| byte.is_ascii_lowercase()) {
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

    /// The heads `scripts/check-codes.py` extracts, read from its HEADS line.
    ///
    /// That line is the one place the head list is written; this parser wants
    /// a single line of the form `HEADS = ("a", "b")`, and panics when the
    /// script no longer offers one, because silently reading no head would
    /// turn the equality assertion below into a tautology.
    fn checker_heads() -> Vec<String> {
        let line = CHECKER
            .lines()
            .find(|line| line.starts_with("HEADS = ("))
            .expect("scripts/check-codes.py must keep its one-line HEADS tuple");
        let heads: Vec<String> = line
            .split('"')
            .skip(1)
            .step_by(2)
            .map(str::to_owned)
            .collect();
        assert!(!heads.is_empty(), "the HEADS line names no head: {line}");
        heads
    }

    #[test]
    fn every_catalogued_code_classifies_to_a_category() {
        let mut seen = 0_usize;
        let mut inputs: Vec<&str> = Vec::new();
        let mut heads: Vec<&str> = Vec::new();
        for line in CATALOGUE.lines() {
            for piece in line.split('`').skip(1).step_by(2) {
                if !code_shaped(piece) {
                    continue;
                }
                seen += 1;
                heads.push(piece.split('.').next().unwrap_or_default());
                assert!(
                    Category::of_code(piece).is_some(),
                    "docs/codes.md lists {piece}, which this build cannot \
classify; add its category segment to Category::of_code"
                );
                if piece.starts_with("input.") && !inputs.contains(&piece) {
                    inputs.push(piece);
                }
            }
        }
        assert!(seen > 80, "the catalogue was read, {seen} codes found");
        heads.sort_unstable();
        heads.dedup();
        let mut expected = checker_heads();
        expected.sort_unstable();
        expected.dedup();
        assert_eq!(
            heads, expected,
            "the heads docs/codes.md documents and the HEADS line of \
scripts/check-codes.py disagree; a head belongs in both, so that the checker \
extracts it and this test classifies it"
        );
        inputs.sort_unstable();
        assert_eq!(
            inputs,
            ["input.over_limit.archive_bytes", "input.unreadable"],
            "the input codes are read by this test, not skipped as prose"
        );
    }

    #[test]
    fn an_unclassified_code_still_fails_but_is_not_called_damage() {
        // Reached only if a code outside the catalogue ever escapes; the
        // status must stay a failure, and the sentence must not claim the
        // package was truncated, which nothing here establishes.
        let failure = Failure::new("archive");
        assert_eq!(failure.category, Category::Package);
        assert_eq!(failure.category.status(), 6);
        assert!(!failure.classified);
        let line = failure.line();
        assert!(line.contains("does not classify"));
        assert!(!line.contains("truncated in transit"));

        let known = Failure::from(ArchiveError::Malformed {
            kind: MalformedKind::CrcMismatch,
            entry: None,
        });
        assert!(known.classified);
        assert!(known.line().contains("truncated in transit"));
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
