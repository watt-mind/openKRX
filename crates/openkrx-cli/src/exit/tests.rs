//! One test per exit-status category, and one that holds the classifier
//! against the whole published catalogue.
//!
//! These are unit tests rather than subprocess tests because the mapping
//! is a pure function of a code string: a subprocess can reach only the
//! codes an archive can be built to produce, and the contract covers every
//! code the crates define.

use super::{Category, Failure};
use openkrx_core::extract::{ExtractLimitKind, UnsupportedEntryKind};
use openkrx_core::{ArchiveError, LimitKind, MalformedKind, PlanError, Structure, UnsupportedKind};

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
    assert!(scoped.line("extract").ends_with("(exit 9)"));
    assert!(
        scoped
            .line("extract")
            .contains("nothing is ever overwritten")
    );
    // Each output code says what happened rather than sharing one
    // sentence that describes all nine conditions at once.
    let missing = Failure::output(super::OUTPUT_DESTINATION_MISSING).line("extract");
    assert!(missing.contains("does not exist"));
    assert!(!missing.contains("overwritten"), "{missing}");
    // And the four codes both writing commands reach name the argument the
    // caller actually typed: telling someone who ran `create` to extract
    // into an empty directory is advice about a command they did not run.
    for (code, extract, create) in [
        (super::OUTPUT_DESTINATION_MISSING, "--into", "--out"),
        (super::OUTPUT_DESTINATION_NOT_A_DIRECTORY, "--into", "--out"),
        (super::OUTPUT_DESTINATION_SYMLINK, "extraction", "--out"),
        (super::OUTPUT_EXISTS, "extract into", "--out"),
    ] {
        let extracting = Failure::output(code).line("extract");
        let creating = Failure::output(code).line("create");
        let repacking = Failure::output(code).line("repack");
        assert!(extracting.contains(extract), "{code}: {extracting}");
        assert!(!extracting.contains("--out"), "{code}: {extracting}");
        for writing in [&creating, &repacking] {
            assert!(writing.contains(create), "{code}: {writing}");
            assert!(!writing.contains("--into"), "{code}: {writing}");
        }
    }
    // The one sentence the two writing commands do not share: `repack`
    // must say that the package being edited is not a legal --out, which
    // is the mistake its own argument shape invites.
    let repacking = Failure::output(super::OUTPUT_EXISTS).line("repack");
    assert!(
        repacking.contains("no package is edited in place"),
        "{repacking}"
    );
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
        assert!(failure.line("create").ends_with("(exit 6)"));
        // Each code carries its own sentence rather than the category's.
        assert!(
            !failure.line("create").contains("truncated in transit"),
            "{code}"
        );
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
fn every_repacking_refusal_classifies_and_explains_itself() {
    // The two categories `repack` adds: a package openkrx cannot re-emit
    // is an unsupported feature, and an edit naming an attachment the
    // package does not hold is a package problem.
    for code in [
        "repack.unsupported.metadata_missing",
        "repack.unsupported.root_prefix",
        "repack.unsupported.metadata_name",
        "repack.unsupported.marker",
        "repack.unsupported.extra_entry",
        "repack.unsupported.unknown_elements",
        "repack.unsupported.opaque_block",
        "repack.unsupported.dispatch_count",
        "repack.unsupported.attachment_reference",
        "repack.unsupported.attachment_entry",
    ] {
        assert_eq!(
            Category::of_code(code),
            Some(Category::Unsupported),
            "{code}"
        );
        let line = Failure::new(code).line("repack");
        assert!(line.ends_with("(exit 7)"), "{line}");
        // Each one says what would be lost or changed, rather than
        // sharing the category's "openkrx does not implement it".
        assert!(
            !line.contains("another reader may open it"),
            "{code}: {line}"
        );
    }
    for code in [
        "repack.invalid.no_such_attachment",
        "repack.invalid.duplicate_target",
        "repack.invalid.inventory_mismatch",
        super::REPACK_SELF_CHECK_FAILED,
    ] {
        assert_eq!(Category::of_code(code), Some(Category::Package), "{code}");
        assert!(
            Failure::new(code).line("repack").ends_with("(exit 6)"),
            "{code}"
        );
    }
    let failure = Failure::repack_self_check_failed();
    assert_eq!(failure.code, "repack.internal.self_check_failed");
    assert!(failure.line("repack").contains("defect in openkrx"));
    assert!(
        !failure.line("repack").contains("manifest"),
        "repack takes no manifest"
    );
}

#[test]
fn a_repacking_refusal_points_at_the_attachment_number_it_concerns() {
    use openkrx_core::repack::{InvalidKind, RepackError, UnsupportedKind};

    let invalid = Failure::from(RepackError::Invalid {
        kind: InvalidKind::NoSuchAttachment,
        number: Some(9),
    });
    assert_eq!(invalid.code, "repack.invalid.no_such_attachment");
    assert_eq!(invalid.attachment_number, Some(9));
    assert_eq!(invalid.entry_index, None);
    assert_eq!(
        invalid.message(),
        "repack.invalid.no_such_attachment at attachment number 9"
    );

    let unsupported = Failure::from(RepackError::Unsupported {
        kind: UnsupportedKind::ExtraEntry,
        entry: Some(4),
        number: None,
    });
    assert_eq!(unsupported.category, Category::Unsupported);
    assert_eq!(unsupported.entry_index, Some(4));

    // The three layers repacking composes keep their own codes, numbers
    // and categories rather than being folded into a repack.* one.
    let read = Failure::from(RepackError::Read(ArchiveError::OverLimit {
        limit: LimitKind::Entries,
        limit_value: 256,
        observed: Some(300),
        entry: None,
    }));
    assert_eq!(read.category, Category::Limit);
    assert_eq!(
        read.message(),
        "archive.over_limit.entries (limit 256, observed 300)"
    );
}

#[test]
fn the_unknown_key_sentence_lists_the_keys_of_the_object_it_names() {
    // The key a caller wrote is never echoed, so the sentence has to carry
    // what they can act on instead: the key set of the object the diagnostic
    // points at. Naming some *other* object's keys would send them looking in
    // the wrong place, which is worse than saying nothing, so each object is
    // held against the array that defines it.
    let line = |field: &'static str, command: &str| {
        Failure::manifest(super::MANIFEST_UNKNOWN_FIELD, field).line(command)
    };
    let metadata_keys = crate::manifest::METADATA_KEYS;
    let cases: [(&str, &str, Vec<&str>); 8] = [
        ("/", "create", crate::manifest::MANIFEST_KEYS.to_vec()),
        ("/", "repack", crate::edits::EDITS_KEYS.to_vec()),
        ("/metadata", "create", metadata_keys.to_vec()),
        (
            "/metadata",
            "repack",
            metadata_keys
                .into_iter()
                .filter(|key| *key != "dispatches")
                .collect(),
        ),
        (
            "/metadata/dispatches",
            "create",
            crate::manifest::DISPATCH_KEYS.to_vec(),
        ),
        (
            "/attachments",
            "create",
            crate::manifest::ATTACHMENT_KEYS.to_vec(),
        ),
        ("/add", "repack", crate::edits::ADD_KEYS.to_vec()),
        ("/replace", "repack", crate::edits::REPLACE_KEYS.to_vec()),
    ];
    for (field, command, keys) in cases {
        let sentence = line(field, command);
        for key in keys {
            assert!(
                sentence.contains(key),
                "the {command} sentence for {field} omits {key}: {sentence}"
            );
        }
        // And never a promise to name the key itself, which openKRX will not
        // do: it is text the caller wrote.
        assert!(sentence.contains("not echoed"), "{field}: {sentence}");
        assert!(sentence.ends_with("(exit 6)"), "{field}: {sentence}");
    }
    // `dispatches` belongs to the manifest's metadata and not to the edits
    // document's, and each sentence says so.
    assert!(line("/metadata", "create").contains("dispatches"));
    assert!(
        line("/metadata", "repack").contains("dispatches is not one of them"),
        "the edits sentence must say why dispatches is absent"
    );
    // A field this build does not list falls back to the general sentence
    // rather than to some other object's keys.
    let unknown = line("/somewhere/else", "repack");
    assert!(
        unknown.contains("compare that object against the schema"),
        "{unknown}"
    );
    assert!(!unknown.contains("whose keys are"), "{unknown}");
}

#[test]
fn an_unreadable_input_says_which_of_repacks_three_files_it_was() {
    // `repack` opens a package, an edits document and a file per edit, so
    // one sentence for all three would leave a caller guessing.
    let line = Failure::unreadable().line("repack");
    assert!(line.contains("--edits document"), "{line}");
    assert!(line.contains("/add/path"), "{line}");
    assert!(line.contains("no field at all means the package"), "{line}");
    // Every other command keeps the category's own sentence.
    assert!(
        Failure::unreadable()
            .line("inspect")
            .contains("the input could not be read"),
        "the reader commands take one input and say so"
    );
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
    assert!(failure.line("create").contains("defect in openkrx"));
    assert!(failure.line("create").contains("nothing was left behind"));
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
    let line = Failure::over_input_cap().line("inspect");
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
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
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
    let line = failure.line("inspect");
    assert!(line.contains("does not classify"));
    assert!(!line.contains("truncated in transit"));

    let known = Failure::from(ArchiveError::Malformed {
        kind: MalformedKind::CrcMismatch,
        entry: None,
    });
    assert!(known.classified);
    assert!(known.line("inspect").contains("truncated in transit"));
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
