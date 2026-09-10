//! Structural checks over synthetic KRX-shaped archives.
//!
//! Every archive is built at run time by the test-only writer in `support/`.
//! No official sample is copied: `docs/profile.md` records that no
//! redistribution licence exists for the primary sources.

mod support;

use openkrx_core::profile::{
    self, CheckId, CheckOutcome, ReferenceResolution, RuleId, StructureReport, StructureSummary,
    codes,
};
use openkrx_core::{Limits, MetadataLimits, archive};
use support::meta::{Attachment, Document, MARKER_CONTENT, METADATA_FILE, canonical_krx, krx};
use support::{Archive, Entry};

fn report(image: &[u8]) -> StructureReport {
    let inventory = archive::inventory(image, &Limits::DEFAULT).expect("archive is accepted");
    profile::check(&inventory, &MetadataLimits::DEFAULT).expect("checks can run")
}

/// A document whose one reference points into `location`.
fn document_at(location: &str) -> Document {
    Document {
        attachments: vec![Attachment::new(1, "synthetic.pdf", location)],
        ..Document::default()
    }
}

// ------------------------------------------------------------ accepted layouts

#[test]
fn the_canonical_layout_passes_every_check_it_can_decide() {
    let report = report(&canonical_krx(&document_at("KRX/OCD/Payload/ID-1")));
    for check in report.checks() {
        assert!(
            matches!(
                check.outcome,
                CheckOutcome::Pass | CheckOutcome::NotApplicable
            ) || check.id == CheckId::DeclaredSize,
            "{} was {:?}",
            check.id.as_str(),
            check.outcome
        );
    }
    // M13 alone stays open: the unit of MERET is unresolved, so a declared and
    // an observed size are reported side by side and never compared.
    assert_eq!(
        report.outcome(CheckId::DeclaredSize),
        CheckOutcome::Unresolved(RuleId::M13)
    );
    assert_eq!(report.summary(), StructureSummary::Unresolved);
}

#[test]
fn a_shorter_root_prefix_is_unresolved_rather_than_wrong() {
    // A19: three primary sources describe three layouts, none superseding.
    for prefix in ["OCD/", ""] {
        let document = document_at(&format!("{prefix}Payload/ID-1"));
        let image = krx(
            prefix,
            METADATA_FILE,
            &document.bytes(),
            &[&format!("{prefix}Payload/ID-1/synthetic.pdf")],
        );
        let report = report(&image);
        assert_eq!(
            report.outcome(CheckId::RootPrefix),
            CheckOutcome::Unresolved(RuleId::A19),
            "for prefix {prefix:?}"
        );
        assert_eq!(
            report.outcome(CheckId::MetadataLocation),
            CheckOutcome::Pass
        );
        assert_eq!(
            report.outcome(CheckId::AttachmentReferences),
            CheckOutcome::Pass
        );
        assert_eq!(
            report.observations().root_prefix.as_deref(),
            Some(prefix.as_bytes())
        );
    }
}

#[test]
fn a_lower_case_metadata_file_name_is_unresolved_rather_than_wrong() {
    // M12: the primary sources spell the file name two different ways.
    let document = document_at("KRX/OCD/Payload/ID-1");
    let image = krx(
        "KRX/OCD/",
        "kuldemeny_meta.xml",
        &document.bytes(),
        &["KRX/OCD/Payload/ID-1/synthetic.pdf"],
    );
    let report = report(&image);
    assert_eq!(
        report.outcome(CheckId::MetadataLocation),
        CheckOutcome::Pass
    );
    assert_eq!(
        report.outcome(CheckId::MetadataFileName),
        CheckOutcome::Unresolved(RuleId::M12)
    );
    assert_eq!(
        report.observations().metadata_entry_name.as_deref(),
        Some(b"KRX/OCD/Metalayer/kuldemeny_meta.xml".as_slice())
    );
}

#[test]
fn a_lower_case_metalayer_directory_is_also_unresolved() {
    let document = document_at("KRX/OCD/Payload/ID-1");
    let image = Archive::of(vec![
        Entry::stored(b"mimetype", MARKER_CONTENT),
        Entry::deflated(b"KRX/OCD/metalayer/KULDEMENY_META.xml", &document.bytes()),
        Entry::stored(b"KRX/OCD/Payload/ID-1/synthetic.pdf", b"payload"),
    ])
    .build();
    assert_eq!(
        report(&image).outcome(CheckId::MetadataFileName),
        CheckOutcome::Unresolved(RuleId::M12)
    );
}

#[test]
fn observations_report_the_header_facts_and_the_attachment_count() {
    let report = report(&canonical_krx(&document_at("KRX/OCD/Payload/ID-1")));
    let observations = report.observations();
    assert_eq!(observations.krx_verzioszam.as_deref(), Some("v0.9"));
    assert_eq!(
        observations
            .forrasrendszer_azonosito
            .map(|value| value.as_str()),
        Some("KER")
    );
    assert_eq!(
        observations.kuldemeny_tipus.map(|value| value.as_str()),
        Some("KULDEMENY")
    );
    assert_eq!(observations.attachment_count, Some(1));
    assert_eq!(observations.metadata_entry_index, Some(1));
    assert!(report.metadata().is_some());
}

#[test]
fn a_resolved_reference_reports_declared_and_observed_sizes_side_by_side() {
    let report = report(&canonical_krx(&document_at("KRX/OCD/Payload/ID-1")));
    let attachment = &report.attachments()[0];
    assert_eq!(attachment.number, 1);
    assert_eq!(
        attachment.declared_path,
        "KRX/OCD/Payload/ID-1/synthetic.pdf"
    );
    assert_eq!(
        attachment.resolution,
        ReferenceResolution::Resolved { entry: 2 }
    );
    assert_eq!(attachment.declared_size_text, "12.5");
    assert_eq!(attachment.declared_size_value, Some(12.5));
    // The declared kilobytes and the observed bytes disagree wildly, and that
    // produces no failure: M13 leaves the unit unresolved.
    assert_eq!(attachment.observed_size, Some(19));
    assert_ne!(
        report.outcome(CheckId::DeclaredSize),
        CheckOutcome::Fail(codes::COUNT_MISMATCH)
    );
}

// ------------------------------------------------------------- metadata location

#[test]
fn an_archive_without_a_metadata_document_fails_the_location_check() {
    let image = Archive::of(vec![Entry::stored(b"mimetype", MARKER_CONTENT)]).build();
    let report = report(&image);
    assert_eq!(
        report.outcome(CheckId::MetadataLocation),
        CheckOutcome::Fail(codes::MISSING)
    );
    assert_eq!(
        report.outcome(CheckId::MetadataParse),
        CheckOutcome::NotApplicable
    );
    assert_eq!(report.summary(), StructureSummary::Inconsistent);
    assert!(report.metadata().is_none());
    assert!(report.attachments().is_empty());
}

#[test]
fn two_metadata_candidates_are_an_ambiguity_not_a_choice() {
    let document = Document::header_only().bytes();
    let image = Archive::of(vec![
        Entry::stored(b"mimetype", MARKER_CONTENT),
        Entry::deflated(b"KRX/OCD/Metalayer/KULDEMENY_META.xml", &document),
        Entry::deflated(b"OCD/Metalayer/KULDEMENY_META.xml", &document),
    ])
    .build();
    assert_eq!(
        report(&image).outcome(CheckId::MetadataLocation),
        CheckOutcome::Fail(codes::AMBIGUOUS_CANDIDATES)
    );
}

#[test]
fn a_metadata_file_outside_a_metalayer_directory_is_not_a_candidate() {
    let document = Document::header_only().bytes();
    let image = Archive::of(vec![
        Entry::stored(b"mimetype", MARKER_CONTENT),
        Entry::deflated(b"KRX/OCD/Payload/KULDEMENY_META.xml", &document),
    ])
    .build();
    assert_eq!(
        report(&image).outcome(CheckId::MetadataLocation),
        CheckOutcome::Fail(codes::MISSING)
    );
}

#[test]
fn a_metadata_file_at_the_archive_root_is_not_a_candidate() {
    let document = Document::header_only().bytes();
    let image = Archive::of(vec![
        Entry::stored(b"mimetype", MARKER_CONTENT),
        Entry::deflated(b"KULDEMENY_META.xml", &document),
    ])
    .build();
    assert_eq!(
        report(&image).outcome(CheckId::MetadataLocation),
        CheckOutcome::Fail(codes::MISSING)
    );
}

// ---------------------------------------------------------------- marker entry

#[test]
fn a_marker_holding_something_else_fails_the_marker_check() {
    let document = Document::header_only().bytes();
    let image = Archive::of(vec![
        Entry::stored(b"mimetype", b"application/zip"),
        Entry::deflated(b"KRX/OCD/Metalayer/KULDEMENY_META.xml", &document),
    ])
    .build();
    assert_eq!(
        report(&image).outcome(CheckId::MarkerEntry),
        CheckOutcome::Fail(codes::MARKER_CONTENT)
    );
}

#[test]
fn an_archive_without_a_marker_fails_the_marker_check() {
    let document = Document::header_only().bytes();
    let image = Archive::of(vec![Entry::deflated(
        b"KRX/OCD/Metalayer/KULDEMENY_META.xml",
        &document,
    )])
    .build();
    assert_eq!(
        report(&image).outcome(CheckId::MarkerEntry),
        CheckOutcome::Fail(codes::MARKER_MISSING)
    );
}

#[test]
fn a_marker_that_is_not_the_first_entry_fails_the_marker_check() {
    let document = Document::header_only().bytes();
    let image = Archive::of(vec![
        Entry::deflated(b"KRX/OCD/Metalayer/KULDEMENY_META.xml", &document),
        Entry::stored(b"mimetype", MARKER_CONTENT),
    ])
    .build();
    assert_eq!(
        report(&image).outcome(CheckId::MarkerEntry),
        CheckOutcome::Fail(codes::MARKER_NOT_FIRST)
    );
}

#[test]
fn a_marker_under_a_directory_prefix_is_unresolved_rather_than_wrong() {
    // A19: one primary source places the marker inside the OCD directory.
    let document = Document::header_only().bytes();
    let image = Archive::of(vec![
        Entry::stored(b"KRX/OCD/mimetype", MARKER_CONTENT),
        Entry::deflated(b"KRX/OCD/Metalayer/KULDEMENY_META.xml", &document),
    ])
    .build();
    assert_eq!(
        report(&image).outcome(CheckId::MarkerEntry),
        CheckOutcome::Unresolved(RuleId::A19)
    );
}

#[test]
fn a_root_marker_is_inspected_even_when_a_prefixed_one_also_exists() {
    // A2 describes a first entry named exactly `mimetype`. When there is one,
    // it is the entry inspected, whatever else ends in the same segment.
    let document = Document::header_only().bytes();
    let image = Archive::of(vec![
        Entry::stored(b"mimetype", MARKER_CONTENT),
        Entry::stored(b"KRX/OCD/mimetype", MARKER_CONTENT),
        Entry::deflated(b"KRX/OCD/Metalayer/KULDEMENY_META.xml", &document),
    ])
    .build();
    assert_eq!(
        report(&image).outcome(CheckId::MarkerEntry),
        CheckOutcome::Pass
    );
}

#[test]
fn a_prefixed_marker_ahead_of_a_root_marker_stays_unresolved() {
    // The reverse ordering: the root marker is not the first entry, so A19 --
    // which leaves the marker's location open -- still prevents a conclusion.
    // A layout no source settles is never reported as a failure.
    let document = Document::header_only().bytes();
    let image = Archive::of(vec![
        Entry::stored(b"KRX/OCD/mimetype", MARKER_CONTENT),
        Entry::stored(b"mimetype", MARKER_CONTENT),
        Entry::deflated(b"KRX/OCD/Metalayer/KULDEMENY_META.xml", &document),
    ])
    .build();
    assert_eq!(
        report(&image).outcome(CheckId::MarkerEntry),
        CheckOutcome::Unresolved(RuleId::A19)
    );
}

#[test]
fn a_deflated_marker_is_not_treated_as_a_finding() {
    // A20 is never asserted: no source states the marker's storage method.
    let document = Document::header_only().bytes();
    let image = Archive::of(vec![
        Entry::deflated(b"mimetype", MARKER_CONTENT),
        Entry::deflated(b"KRX/OCD/Metalayer/KULDEMENY_META.xml", &document),
    ])
    .build();
    assert_eq!(
        report(&image).outcome(CheckId::MarkerEntry),
        CheckOutcome::Pass
    );
}

// ------------------------------------------------------------------ references

#[test]
fn a_reference_to_a_missing_entry_fails() {
    let document = document_at("KRX/OCD/Payload/ID-9");
    let image = krx(
        "KRX/OCD/",
        METADATA_FILE,
        &document.bytes(),
        &["KRX/OCD/Payload/ID-1/synthetic.pdf"],
    );
    let report = report(&image);
    assert_eq!(
        report.outcome(CheckId::AttachmentReferences),
        CheckOutcome::Fail(codes::REFERENCE_MISSING_ENTRY)
    );
    assert_eq!(
        report.attachments()[0].resolution,
        ReferenceResolution::Missing
    );
    assert_eq!(report.attachments()[0].observed_size, None);
}

#[test]
fn a_reference_that_only_resolves_after_a_prefix_swap_is_unresolved() {
    // M14: nothing says whether ELHELYEZKEDES is archive-root-relative.
    let document = document_at("KRX/OCD/Payload/ID-1");
    let image = krx(
        "OCD/",
        METADATA_FILE,
        &document.bytes(),
        &["OCD/Payload/ID-1/synthetic.pdf"],
    );
    let report = report(&image);
    assert_eq!(
        report.outcome(CheckId::AttachmentReferences),
        CheckOutcome::Unresolved(RuleId::M14)
    );
    assert_eq!(
        report.attachments()[0].resolution,
        ReferenceResolution::PrefixVariant { entry: 2 }
    );
    assert_eq!(report.attachments()[0].observed_size, Some(19));
}

#[test]
fn an_unprefixed_reference_resolves_against_a_prefixed_archive_as_a_variant() {
    let document = document_at("Payload/ID-1");
    let image = krx(
        "KRX/OCD/",
        METADATA_FILE,
        &document.bytes(),
        &["KRX/OCD/Payload/ID-1/synthetic.pdf"],
    );
    assert_eq!(
        report(&image).outcome(CheckId::AttachmentReferences),
        CheckOutcome::Unresolved(RuleId::M14)
    );
}

#[test]
fn two_references_sharing_an_attachment_number_fail() {
    let mut second = Attachment::new(1, "second.pdf", "KRX/OCD/Payload/ID-2");
    second.number = "1".to_owned();
    let document = Document {
        declared_count: Some("2".to_owned()),
        attachments: vec![
            Attachment::new(1, "synthetic.pdf", "KRX/OCD/Payload/ID-1"),
            second,
        ],
        ..Document::default()
    };
    let image = krx(
        "KRX/OCD/",
        METADATA_FILE,
        &document.bytes(),
        &[
            "KRX/OCD/Payload/ID-1/synthetic.pdf",
            "KRX/OCD/Payload/ID-2/second.pdf",
        ],
    );
    assert_eq!(
        report(&image).outcome(CheckId::AttachmentUniqueness),
        CheckOutcome::Fail(codes::REFERENCE_DUPLICATE)
    );
}

#[test]
fn two_references_sharing_a_joined_path_fail() {
    let document = Document {
        declared_count: Some("2".to_owned()),
        attachments: vec![
            Attachment::new(1, "synthetic.pdf", "KRX/OCD/Payload/ID-1"),
            Attachment::new(2, "synthetic.pdf", "KRX/OCD/Payload/ID-1"),
        ],
        ..Document::default()
    };
    let image = krx(
        "KRX/OCD/",
        METADATA_FILE,
        &document.bytes(),
        &["KRX/OCD/Payload/ID-1/synthetic.pdf"],
    );
    assert_eq!(
        report(&image).outcome(CheckId::AttachmentUniqueness),
        CheckOutcome::Fail(codes::REFERENCE_DUPLICATE)
    );
}

#[test]
fn a_shared_file_name_in_different_payload_directories_is_accepted() {
    // A6: two attachments may share a file name in different subdirectories.
    let document = Document {
        declared_count: Some("2".to_owned()),
        attachments: vec![
            Attachment::new(1, "synthetic.pdf", "KRX/OCD/Payload/ID-1"),
            Attachment::new(2, "synthetic.pdf", "KRX/OCD/Payload/ID-2"),
        ],
        ..Document::default()
    };
    let image = krx(
        "KRX/OCD/",
        METADATA_FILE,
        &document.bytes(),
        &[
            "KRX/OCD/Payload/ID-1/synthetic.pdf",
            "KRX/OCD/Payload/ID-2/synthetic.pdf",
        ],
    );
    let report = report(&image);
    assert_eq!(
        report.outcome(CheckId::AttachmentUniqueness),
        CheckOutcome::Pass
    );
    assert_eq!(
        report.outcome(CheckId::AttachmentReferences),
        CheckOutcome::Pass
    );
}

#[test]
fn a_declared_count_disagreeing_with_the_list_fails() {
    // M7: the schema requires both, so the two can disagree.
    let document = Document {
        declared_count: Some("4".to_owned()),
        ..document_at("KRX/OCD/Payload/ID-1")
    };
    let image = krx(
        "KRX/OCD/",
        METADATA_FILE,
        &document.bytes(),
        &["KRX/OCD/Payload/ID-1/synthetic.pdf"],
    );
    let report = report(&image);
    assert_eq!(
        report.outcome(CheckId::AttachmentCount),
        CheckOutcome::Fail(codes::COUNT_MISMATCH)
    );
    assert_eq!(report.summary(), StructureSummary::Inconsistent);
}

#[test]
fn a_document_declaring_no_count_leaves_the_count_check_inapplicable() {
    let document = Document {
        declared_count: None,
        ..document_at("KRX/OCD/Payload/ID-1")
    };
    let image = krx(
        "KRX/OCD/",
        METADATA_FILE,
        &document.bytes(),
        &["KRX/OCD/Payload/ID-1/synthetic.pdf"],
    );
    assert_eq!(
        report(&image).outcome(CheckId::AttachmentCount),
        CheckOutcome::NotApplicable
    );
}

// --------------------------------------------------------------- unresolved M11

#[test]
fn an_omitted_schema_required_element_is_unresolved_rather_than_wrong() {
    // M11: an official example omits TESZT and MELLEKLET_LEIRASA and writes a
    // non-numeric MERET. None of the three may be reported as invalid.
    let mut attachment = Attachment::new(1, "synthetic.pdf", "KRX/OCD/Payload/ID-1");
    attachment.description = None;
    attachment.size = "60110 bytes".to_owned();
    let document = Document {
        test: None,
        attachments: vec![attachment],
        ..Document::default()
    };
    let image = krx(
        "KRX/OCD/",
        METADATA_FILE,
        &document.bytes(),
        &["KRX/OCD/Payload/ID-1/synthetic.pdf"],
    );
    let report = report(&image);
    assert_eq!(
        report.outcome(CheckId::SchemaOptionalFields),
        CheckOutcome::Unresolved(RuleId::M11)
    );
    assert_eq!(report.outcome(CheckId::MetadataParse), CheckOutcome::Pass);
    assert_eq!(report.summary(), StructureSummary::Unresolved);
    assert_eq!(report.attachments()[0].declared_size_value, None);
}

// ---------------------------------------------------------------- parse failure

#[test]
fn a_metadata_document_that_does_not_parse_fails_with_the_parser_code() {
    let document = Document {
        test: Some("igen".to_owned()),
        ..Document::default()
    };
    let image = krx("KRX/OCD/", METADATA_FILE, &document.bytes(), &[]);
    let report = report(&image);
    assert_eq!(
        report.outcome(CheckId::MetadataParse),
        CheckOutcome::Fail("metadata.malformed.boolean")
    );
    assert_eq!(
        report.outcome(CheckId::AttachmentReferences),
        CheckOutcome::NotApplicable
    );
    assert!(report.metadata().is_none());
}

#[test]
fn the_caller_can_tighten_the_metadata_limits_for_a_structural_check() {
    let image = canonical_krx(&document_at("KRX/OCD/Payload/ID-1"));
    let inventory = archive::inventory(&image, &Limits::DEFAULT).expect("accepted");
    let mut limits = MetadataLimits::DEFAULT;
    limits.max_depth = 2;
    let report = profile::check(&inventory, &limits).expect("checks can run");
    assert_eq!(
        report.outcome(CheckId::MetadataParse),
        CheckOutcome::Fail("metadata.over_limit.depth")
    );
}

// ------------------------------------------------------------------- inventory

#[test]
fn every_check_is_reported_exactly_once_in_the_documented_order() {
    let report = report(&canonical_krx(&document_at("KRX/OCD/Payload/ID-1")));
    let ids: Vec<CheckId> = report.checks().iter().map(|check| check.id).collect();
    assert_eq!(ids, CheckId::ORDER.to_vec());
    assert_eq!(ids.len(), 11);
    let names: Vec<&str> = ids.iter().map(|id| id.as_str()).collect();
    assert_eq!(names[0], "metadata_location");
    assert_eq!(names[10], "declared_size");
}

#[test]
fn every_unresolved_rule_id_prints_the_identifier_the_evidence_uses() {
    for (rule, text) in [
        (RuleId::A19, "A19"),
        (RuleId::A20, "A20"),
        (RuleId::A21, "A21"),
        (RuleId::A22, "A22"),
        (RuleId::M11, "M11"),
        (RuleId::M12, "M12"),
        (RuleId::M13, "M13"),
        (RuleId::M14, "M14"),
        (RuleId::M15, "M15"),
    ] {
        assert_eq!(rule.as_str(), text);
    }
}

#[test]
fn the_summary_is_consistent_only_when_nothing_failed_or_stayed_open() {
    // A header-only document under the canonical layout leaves nothing open:
    // there is no attachment, so M13 and M14 do not arise.
    let image = krx(
        "KRX/OCD/",
        METADATA_FILE,
        &Document::header_only().bytes(),
        &[],
    );
    let report = report(&image);
    assert_eq!(report.summary(), StructureSummary::Consistent);
    // And Consistent is still not a conformance claim; it says only that no
    // check failed and none was left unresolved. The crate advertises reading,
    // protected extraction, deterministic creation and deterministic
    // repacking, and nothing else: no verdict and no verification anywhere,
    // and a package the writer produces is layout-consistent rather than
    // conforming.
    assert_eq!(
        openkrx_core::capabilities().operations,
        [
            "inspect",
            "list",
            "validate-structure",
            "extract",
            "create",
            "repack"
        ]
    );
    assert_eq!(openkrx_core::capabilities().stage, "reader-writer");
}

#[test]
fn a_report_answers_for_a_check_it_never_recorded() {
    let image = Archive::of(vec![Entry::stored(b"mimetype", MARKER_CONTENT)]).build();
    assert_eq!(
        report(&image).outcome(CheckId::DeclaredSize),
        CheckOutcome::NotApplicable
    );
}

#[test]
fn a_profile_error_carries_the_underlying_archive_code() {
    use std::error::Error;

    let error = profile::ProfileError::from(openkrx_core::ArchiveError::NoSuchEntry { entry: 7 });
    assert_eq!(error.code(), "archive.no_such_entry");
    assert_eq!(error.to_string(), "archive.no_such_entry at entry 7");
    assert!(error.source().is_some());
}

#[test]
fn no_truncation_of_a_krx_shaped_archive_ever_panics() {
    let image = canonical_krx(&document_at("KRX/OCD/Payload/ID-1"));
    for length in 0..image.len() {
        if let Ok(inventory) = archive::inventory(&image[..length], &Limits::DEFAULT) {
            let _ = profile::check(&inventory, &MetadataLimits::DEFAULT);
        }
    }
}

// ------------------------------------- each half of the M11 completeness rule

#[test]
fn an_absent_test_flag_alone_leaves_the_optional_field_check_undecided() {
    // M11 is undecided when *any* element one official example omits is
    // missing. Here every attachment is complete and only `TESZT` is absent,
    // so a check that required both conditions and one that required either
    // would disagree: this holds it to both.
    let document = Document {
        test: None,
        ..document_at("KRX/OCD/Payload/ID-1")
    };
    let report = report(&krx(
        "KRX/OCD/",
        METADATA_FILE,
        &document.bytes(),
        &["KRX/OCD/Payload/ID-1/synthetic.pdf"],
    ));
    assert_eq!(
        report.outcome(CheckId::SchemaOptionalFields),
        CheckOutcome::Unresolved(RuleId::M11)
    );
}

#[test]
fn an_attachment_missing_either_optional_element_leaves_the_check_undecided() {
    // The two halves of the per-attachment condition, one at a time: a
    // reference with no description, and one whose MERET carries no number.
    // Each on its own must leave M11 undecided.
    for attachment in [
        Attachment {
            description: None,
            ..Attachment::new(1, "synthetic.pdf", "KRX/OCD/Payload/ID-1")
        },
        Attachment {
            size: "not a number".to_owned(),
            ..Attachment::new(1, "synthetic.pdf", "KRX/OCD/Payload/ID-1")
        },
    ] {
        let document = Document {
            attachments: vec![attachment],
            ..Document::default()
        };
        let report = report(&krx(
            "KRX/OCD/",
            METADATA_FILE,
            &document.bytes(),
            &["KRX/OCD/Payload/ID-1/synthetic.pdf"],
        ));
        assert_eq!(
            report.outcome(CheckId::SchemaOptionalFields),
            CheckOutcome::Unresolved(RuleId::M11),
            "an incomplete reference leaves M11 undecided"
        );
    }

    // The complete document is the control: both halves present is the only
    // combination that passes.
    let complete = report(&krx(
        "KRX/OCD/",
        METADATA_FILE,
        &document_at("KRX/OCD/Payload/ID-1").bytes(),
        &["KRX/OCD/Payload/ID-1/synthetic.pdf"],
    ));
    assert_eq!(
        complete.outcome(CheckId::SchemaOptionalFields),
        CheckOutcome::Pass
    );
}

#[test]
fn a_reference_resolves_through_a_root_prefix_other_than_the_observed_one() {
    // The observed root prefix is `KRX/OCD/`, and the declared path uses it,
    // but the entry is stored under `OCD/`. Resolution must try the other
    // known prefixes as well as the observed one, or this reference reads as
    // missing rather than as the prefix variant rule M14 describes.
    let document = document_at("KRX/OCD/Payload/ID-1");
    let image = Archive::of(vec![
        Entry::stored(b"mimetype", MARKER_CONTENT),
        Entry::deflated(
            format!("KRX/OCD/Metalayer/{METADATA_FILE}").as_bytes(),
            &document.bytes(),
        ),
        Entry::stored(b"OCD/Payload/ID-1/synthetic.pdf", b"synthetic payload 0"),
    ])
    .build();
    let report = report(&image);
    assert_eq!(
        report.attachments()[0].resolution,
        ReferenceResolution::PrefixVariant { entry: 2 },
        "the entry is found under a known prefix that is not the observed one"
    );
    assert_eq!(
        report.outcome(CheckId::AttachmentReferences),
        CheckOutcome::Unresolved(RuleId::M14)
    );
}
