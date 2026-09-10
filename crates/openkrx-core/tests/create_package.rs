//! What the deterministic writer produces, and what reading it back reports.
//!
//! Every document here is authored in this file or by the test-only writer in
//! `support/`. No official sample is copied: `docs/profile.md` records that no
//! redistribution licence exists for the primary sources, and nothing in this
//! repository claims that a written package is accepted by any real service.

mod support;

use openkrx_core::create::{
    self, AttachmentInput, FixedTimestamp, Layout, MARKER_CONTENT, MARKER_NAME, METADATA_NAME,
    PackageSpec,
};
use openkrx_core::extract::{self, ExtractLimits};
use openkrx_core::metadata::{Metadata, TARGET_NAMESPACE};
use openkrx_core::profile::{CheckId, CheckOutcome, ReferenceResolution, RuleId, StructureSummary};
use openkrx_core::{Limits, MetadataLimits, archive, metadata};
use support::meta::Document;

/// The document a caller starts from: a header and one empty dispatch block.
fn metadata_with_dispatch() -> Metadata {
    let document = Document {
        declared_count: None,
        attachments: Vec::new(),
        handling_instructions: true,
        ..Document::default()
    };
    parse(&document.bytes())
}

/// A header-only document, with no `EXPEDIALASOK` at all.
fn header_only() -> Metadata {
    parse(&Document::header_only().bytes())
}

#[track_caller]
fn parse(document: &[u8]) -> Metadata {
    metadata::parse(document, &MetadataLimits::DEFAULT).expect("the document parses")
}

/// A package with `count` described attachments, and the bytes of each.
fn spec_with(count: usize) -> PackageSpec {
    let attachments = (0..count)
        .map(|index| {
            AttachmentInput::described(
                format!("melleklet-{index}.pdf"),
                payload(index),
                format!("árvíztűrő description {index}"),
            )
        })
        .collect();
    PackageSpec::with_attachments(metadata_with_dispatch(), attachments)
}

/// Deterministic, poorly compressible attachment bytes.
fn payload(index: usize) -> Vec<u8> {
    support::pseudo_random(700 + index * 331)
}

#[track_caller]
fn write(spec: &PackageSpec) -> Vec<u8> {
    create::package(spec, &Limits::DEFAULT).expect("the package is written")
}

/// The names of the entries an image holds, in central-directory order.
fn names(image: &[u8]) -> Vec<String> {
    let inventory = archive::inventory(image, &Limits::DEFAULT).expect("the image is an archive");
    inventory
        .entries()
        .iter()
        .map(|entry| String::from_utf8(entry.name_bytes().to_vec()).expect("names are UTF-8"))
        .collect()
}

// --------------------------------------------------------------- the layout

#[test]
fn the_entries_are_the_documented_layout_in_a_fixed_order() {
    for count in [0, 1, 3] {
        assert_eq!(
            names(&write(&spec_with(count))),
            [
                MARKER_NAME.to_owned(),
                METADATA_NAME.to_owned(),
                "KRX/OCD/Payload/ID-1/melleklet-0.pdf".to_owned(),
                "KRX/OCD/Payload/ID-2/melleklet-1.pdf".to_owned(),
                "KRX/OCD/Payload/ID-3/melleklet-2.pdf".to_owned(),
            ][..count + 2],
            "{count} attachments"
        );
    }
}

#[test]
fn the_marker_is_the_first_entry_stored_verbatim() {
    let image = write(&spec_with(1));
    let inventory = archive::inventory(&image, &Limits::DEFAULT).expect("the image is an archive");
    let marker = inventory.entries().first().expect("the marker is first");
    assert_eq!(marker.name_bytes(), MARKER_NAME.as_bytes());
    assert_eq!(marker.method(), 0, "the marker is stored, not deflated");
    assert_eq!(
        inventory.entry_bytes(0).expect("the marker decodes"),
        MARKER_CONTENT
    );
    // Every other entry is deflated at the one fixed level.
    for index in 1..inventory.len() as u32 {
        assert_eq!(inventory.entries()[index as usize].method(), 8, "{index}");
    }
}

#[test]
fn every_entry_declares_a_utf8_name_and_a_regular_file() {
    let image = write(&spec_with(2));
    let inventory = archive::inventory(&image, &Limits::DEFAULT).expect("the image is an archive");
    for entry in inventory.entries() {
        assert!(entry.utf8_flag(), "bit 11 is set on every name");
        assert_eq!(entry.flags(), 1 << 11, "no other flag is ever written");
        assert_eq!(entry.version_made_by(), (3 << 8) | 20);
        assert_eq!(entry.external_attributes(), 0o100_644 << 16);
        assert_eq!(entry.kind(), archive::EntryKind::RegularFile);
    }
    // No directory entry is written, so the plan holds exactly the files.
    let plan = extract::plan(&inventory, &ExtractLimits::DEFAULT).expect("the package extracts");
    assert_eq!(plan.items().len(), inventory.len());
}

#[test]
fn the_document_is_the_shape_the_examples_show() {
    let image = write(&spec_with(1));
    let inventory = archive::inventory(&image, &Limits::DEFAULT).expect("the image is an archive");
    let document = inventory.entry_bytes(1).expect("the document decodes");
    let text = String::from_utf8(document).expect("the document is UTF-8");
    assert!(text.starts_with(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><ns2:KULDEMENY "
    ));
    assert!(text.contains(&format!("xmlns:ns2=\"{TARGET_NAMESPACE}\"")));
    // M8: this one element is declared unqualified, so it carries no prefix.
    assert!(text.contains("<KEZELESI_UTASITASOK></KEZELESI_UTASITASOK>"));
    assert!(!text.contains("<!DOCTYPE"));
    assert!(!text.contains("<ns2:KEZELESI_UTASITASOK"));
    // M2 and M3: the sequence the schema fixes, with no whitespace between.
    let order: Vec<&str> = ["<ns2:FEJRESZ>", "<ns2:EXPEDIALASOK>", "<ns2:MELLEKLET>"]
        .into_iter()
        .collect();
    let mut cursor = 0;
    for element in order {
        let position = text[cursor..]
            .find(element)
            .unwrap_or_else(|| panic!("{element} is written"));
        cursor += position;
    }
}

#[test]
fn the_derived_references_describe_where_each_attachment_went() {
    let image = write(&spec_with(2));
    let inventory = archive::inventory(&image, &Limits::DEFAULT).expect("the image is an archive");
    let document = parse(&inventory.entry_bytes(1).expect("the document decodes"));
    let references: Vec<_> = document.attachments().collect();
    assert_eq!(references.len(), 2);
    for (index, reference) in references.iter().enumerate() {
        assert_eq!(reference.number, index as i64 + 1);
        assert_eq!(reference.file_name, format!("melleklet-{index}.pdf"));
        assert_eq!(
            reference.location,
            format!("KRX/OCD/Payload/ID-{}", index + 1)
        );
        // M6: kilobytes, rounded up, as a numeric string. M13 leaves what a
        // service expects there unresolved, and the check keeps saying so.
        let expected = payload(index).len().div_ceil(1024);
        assert_eq!(reference.size_text, expected.to_string());
        assert_eq!(reference.size_value, Some(expected as f64));
        assert_eq!(
            reference.declared_path(),
            format!("KRX/OCD/Payload/ID-{}/melleklet-{index}.pdf", index + 1)
        );
    }
    assert_eq!(
        document.dispatches[0].declared_attachment_count,
        Some(2),
        "MELLEKLETEK_SZAMA is derived from the attachments"
    );
}

// ----------------------------------------------------------- the round trip

#[test]
fn a_written_package_fails_no_structural_check() {
    for count in [0, 1, 3] {
        let image = write(&spec_with(count));
        let report = create::verify_round_trip(&image, &Limits::DEFAULT, &MetadataLimits::DEFAULT)
            .expect("the package reads back");
        for check in report.checks() {
            assert!(
                !matches!(check.outcome, CheckOutcome::Fail(_)),
                "{count} attachments: {} was {:?}",
                check.id.as_str(),
                check.outcome
            );
        }
        assert_eq!(report.summary(), StructureSummary::Unresolved);
    }
}

#[test]
fn exactly_two_rules_stay_unresolved_over_a_written_package() {
    let image = write(&spec_with(1));
    let report = create::verify_round_trip(&image, &Limits::DEFAULT, &MetadataLimits::DEFAULT)
        .expect("the package reads back");
    let unresolved: Vec<(&str, RuleId)> = report
        .checks()
        .iter()
        .filter_map(|check| match check.outcome {
            CheckOutcome::Unresolved(rule) => Some((check.id.as_str(), rule)),
            _ => None,
        })
        .collect();
    // A19: the marker sits under KRX/OCD/, which is one of the three layouts
    // the primary sources describe and none of them settles. M13: the unit and
    // rounding of MERET. Nothing the writer can do resolves either.
    assert_eq!(
        unresolved,
        [
            ("marker_entry", RuleId::A19),
            ("declared_size", RuleId::M13)
        ]
    );
    assert_eq!(report.outcome(CheckId::RootPrefix), CheckOutcome::Pass);
    assert_eq!(
        report.outcome(CheckId::MetadataFileName),
        CheckOutcome::Pass
    );
    assert_eq!(
        report.outcome(CheckId::SchemaOptionalFields),
        CheckOutcome::Pass
    );
    assert_eq!(
        report.attachments()[0].resolution,
        ReferenceResolution::Resolved { entry: 2 }
    );
}

#[test]
fn a_package_written_without_descriptions_reports_m11_instead() {
    let spec = PackageSpec::with_attachments(
        metadata_with_dispatch(),
        vec![AttachmentInput::new("a.pdf", b"pdf".to_vec())],
    );
    let report =
        create::verify_round_trip(&write(&spec), &Limits::DEFAULT, &MetadataLimits::DEFAULT)
            .expect("the package reads back");
    assert_eq!(
        report.outcome(CheckId::SchemaOptionalFields),
        CheckOutcome::Unresolved(RuleId::M11),
        "an absent MELLEKLET_LEIRASA is what M11 records, not a failure"
    );
}

#[test]
fn every_attachment_reads_back_byte_identically() {
    let spec = spec_with(3);
    let image = write(&spec);
    let inventory = archive::inventory(&image, &Limits::DEFAULT).expect("the image is an archive");
    for (index, attachment) in spec.attachments.iter().enumerate() {
        let entry = index as u32 + 2;
        assert_eq!(
            inventory.entry_bytes(entry).expect("the entry decodes"),
            attachment.bytes,
            "attachment {index}"
        );
        assert_eq!(
            inventory.entries()[entry as usize].decoded_size(),
            attachment.bytes.len() as u64
        );
    }
}

#[test]
fn an_incompressible_attachment_still_reads_back_unchanged() {
    // Deflate can expand such bytes; the reader is the gate either way.
    let bytes = support::pseudo_random(64 * 1024);
    let spec = PackageSpec::with_attachments(
        metadata_with_dispatch(),
        vec![AttachmentInput::new("noise.bin", bytes.clone())],
    );
    let image = write(&spec);
    let inventory = archive::inventory(&image, &Limits::DEFAULT).expect("the image is an archive");
    assert_eq!(inventory.entry_bytes(2).expect("the entry decodes"), bytes);
}

#[test]
fn a_document_read_back_writes_the_same_package_again() {
    // parse ∘ serialise is a fixed point: the references the writer derived are
    // exactly the ones it accepts as caller-supplied, so nothing drifts when a
    // package is read and written again.
    let spec = spec_with(2);
    let image = write(&spec);
    let inventory = archive::inventory(&image, &Limits::DEFAULT).expect("the image is an archive");
    let document = parse(&inventory.entry_bytes(1).expect("the document decodes"));
    let again = PackageSpec::with_attachments(document, spec.attachments.clone());
    assert_eq!(write(&again), image);
}

#[test]
fn a_document_with_no_dispatch_survives_the_round_trip_unchanged() {
    let metadata = header_only();
    let image = write(&PackageSpec::new(metadata.clone()));
    let inventory = archive::inventory(&image, &Limits::DEFAULT).expect("the image is an archive");
    let written = parse(&inventory.entry_bytes(1).expect("the document decodes"));
    assert_eq!(written, metadata);
}

#[test]
fn every_optional_element_survives_the_round_trip() {
    // Authored here rather than by the synthetic builder, which writes no
    // optional header element: this is the representative model M3 and M5
    // describe, with every optional field and every marker block present.
    let document = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<ns2:KULDEMENY xmlns:ns2=\"{TARGET_NAMESPACE}\">\
<ns2:FEJRESZ>\
<ns2:KRX_VERZIOSZAM>v0.9</ns2:KRX_VERZIOSZAM>\
<ns2:FORRASRENDSZER_AZONOSITO>POSTA</ns2:FORRASRENDSZER_AZONOSITO>\
<ns2:KULDEMENY_AZONOSITO>AZ-1 &amp; AZ-2</ns2:KULDEMENY_AZONOSITO>\
<ns2:KULDEMENY_LETREHOZASANAK_IDEJE>2026-01-02T03:04:05.000+01:00\
</ns2:KULDEMENY_LETREHOZASANAK_IDEJE>\
<ns2:KULDEMENY_TIPUS>NYUGTA</ns2:KULDEMENY_TIPUS>\
<ns2:TESZT>true</ns2:TESZT>\
<ns2:VONALKOD>1234567890</ns2:VONALKOD>\
<ns2:KULDEMENY_HIVATKOZASI_AZONOSITO>HIV-1</ns2:KULDEMENY_HIVATKOZASI_AZONOSITO>\
<ns2:HIBAKOD>0</ns2:HIBAKOD>\
<ns2:KULDEMENY_MEGJEGYZES>&lt;note&gt; &amp; more</ns2:KULDEMENY_MEGJEGYZES>\
</ns2:FEJRESZ>\
<ns2:ERKEZTETES></ns2:ERKEZTETES>\
<ns2:BONTASOK></ns2:BONTASOK>\
<ns2:TERTIVEVENY></ns2:TERTIVEVENY>\
</ns2:KULDEMENY>"
    );
    let metadata = parse(document.as_bytes());
    assert!(metadata.receipt_present && metadata.openings_present);
    assert!(metadata.return_receipt_present);
    let image = write(&PackageSpec::new(metadata.clone()));
    let inventory = archive::inventory(&image, &Limits::DEFAULT).expect("the image is an archive");
    let written = parse(&inventory.entry_bytes(1).expect("the document decodes"));
    assert_eq!(written, metadata, "every element and its text is preserved");
    // The escaped characters survive as characters, not as their escapes.
    assert_eq!(written.header.note.as_deref(), Some("<note> & more"));
}

#[test]
fn an_absent_teszt_element_stays_absent() {
    let document = Document {
        test: None,
        declared_count: None,
        attachments: Vec::new(),
        handling_instructions: false,
        ..Document::default()
    };
    let metadata = parse(&document.bytes());
    assert!(!metadata.header.test_present);
    let image = write(&PackageSpec::new(metadata.clone()));
    let inventory = archive::inventory(&image, &Limits::DEFAULT).expect("the image is an archive");
    let written = parse(&inventory.entry_bytes(1).expect("the document decodes"));
    assert_eq!(written, metadata);
    assert!(
        !String::from_utf8(inventory.entry_bytes(1).expect("decodes"))
            .expect("UTF-8")
            .contains("TESZT"),
        "M11: an absent element is written as absent, not defaulted"
    );
}

#[test]
fn the_optional_attachment_elements_are_written_when_supplied() {
    let mut attachment = AttachmentInput::described("a.pdf", b"payload".to_vec(), "leírás");
    attachment.quantity = Some("3".to_owned());
    attachment.quantity_unit = Some("db".to_owned());
    let image = write(&PackageSpec::with_attachments(
        metadata_with_dispatch(),
        vec![attachment],
    ));
    let inventory = archive::inventory(&image, &Limits::DEFAULT).expect("the image is an archive");
    let document = parse(&inventory.entry_bytes(1).expect("the document decodes"));
    let reference = document.attachments().next().expect("one reference");
    assert_eq!(reference.description.as_deref(), Some("leírás"));
    assert_eq!(reference.quantity.as_deref(), Some("3"));
    assert_eq!(reference.quantity_unit.as_deref(), Some("db"));
}

// ------------------------------------------------------------- determinism

#[test]
fn the_same_request_writes_the_same_bytes() {
    let spec = spec_with(3);
    let first = write(&spec);
    let second = write(&spec);
    assert_eq!(first, second);
    // A second, independently built request writes the same bytes too, so
    // nothing about a particular value's identity reaches the output.
    assert_eq!(first, write(&spec_with(3)));
}

#[test]
fn one_different_attachment_byte_changes_the_output() {
    let spec = spec_with(1);
    let mut changed = spec.clone();
    changed.attachments[0].bytes[0] ^= 0xff;
    assert_ne!(write(&spec), write(&changed));
}

#[test]
fn the_timestamp_is_the_callers_and_reaches_every_record() {
    let mut spec = spec_with(1);
    assert_eq!(spec.timestamp, FixedTimestamp::EPOCH);
    let epoch = write(&spec);
    spec.timestamp = FixedTimestamp::from_parts(2026, 9, 10, 11, 12, 13).expect("a valid time");
    let stamped = write(&spec);
    assert_ne!(epoch, stamped);
    let date = (2026_u16 - 1980) << 9 | (9 << 5) | 10;
    let time = (11_u16 << 11) | (12 << 5) | 6;
    let occurrences = stamped
        .windows(4)
        .filter(|window| window[..2] == time.to_le_bytes() && window[2..] == date.to_le_bytes())
        .count();
    assert_eq!(
        occurrences,
        2 * 3,
        "one local header and one central record per entry carry it"
    );
    // Rounding is downward to the two-second resolution the field holds.
    assert_eq!(
        FixedTimestamp::from_parts(2026, 9, 10, 11, 12, 12),
        FixedTimestamp::from_parts(2026, 9, 10, 11, 12, 13)
    );
    assert_eq!(
        FixedTimestamp::from_dos(date, time),
        FixedTimestamp::from_parts(2026, 9, 10, 11, 12, 13).expect("a valid time")
    );
}

#[test]
fn the_layout_is_the_one_documented_layout() {
    assert_eq!(Layout::default(), Layout::CanonicalDocumented);
    let spec = PackageSpec::new(header_only());
    assert_eq!(spec.layout, Layout::CanonicalDocumented);
    assert_eq!(spec.attachments, Vec::new());
}

#[test]
fn the_image_carries_no_extra_field_comment_or_data_descriptor() {
    let image = write(&spec_with(2));
    let inventory = archive::inventory(&image, &Limits::DEFAULT).expect("the image is an archive");
    // The archive layer refuses an unclaimed byte, so exact coverage already
    // proves there is no gap; these assert the fields themselves.
    for entry in inventory.entries() {
        assert_eq!(entry.flags() & (1 << 3), 0, "no data descriptor");
        assert_eq!(entry.compressed_size(), {
            let start = entry.local_header_offset() as usize;
            let name = u16::from_le_bytes([image[start + 26], image[start + 27]]) as usize;
            let extra = u16::from_le_bytes([image[start + 28], image[start + 29]]) as usize;
            assert_eq!(extra, 0, "no local extra field");
            assert_eq!(name, entry.name_bytes().len());
            entry.compressed_size()
        });
    }
    let comment = u16::from_le_bytes([image[image.len() - 2], image[image.len() - 1]]);
    assert_eq!(comment, 0, "the archive comment is empty");
}
