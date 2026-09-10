//! What repacking preserves, what it changes, and what it writes.
//!
//! Every package here is written by `create::package` in this file, from
//! values authored in this repository. No official sample is copied, and
//! nothing asserts that a repacked package conforms to anything.

mod support;

use openkrx_core::create::{self, AttachmentInput, FixedTimestamp, PackageSpec};
use openkrx_core::draft::{self, HeaderDraft};
use openkrx_core::metadata::{ConsignmentKind, Metadata, SourceSystem};
use openkrx_core::profile::CheckOutcome;
use openkrx_core::repack::{self, AttachmentAddition, AttachmentReplacement, Edits, HeaderField};
use openkrx_core::{Limits, MetadataLimits, archive, metadata};
use support::{Archive, Entry};

/// The timestamp every package in this file carries.
const STAMP: FixedTimestamp = FixedTimestamp::EPOCH;

/// A document with a header and one empty dispatch block.
///
/// Built through the crate's own constructors, so it carries nothing the
/// writer cannot reproduce: no unknown element, no opaque block and no
/// unqualified handling-instruction element.
fn document() -> Metadata {
    let header = HeaderDraft {
        version: "0.9".to_owned(),
        source_system: SourceSystem::Ker,
        consignment_id: "SYNTHETIC-CONSIGNMENT-1".to_owned(),
        created_at_text: "2026-01-02T03:04:05".to_owned(),
        consignment_kind: ConsignmentKind::Kuldemeny,
        test: true,
        barcode: None,
        reference_id: None,
        error_code: None,
        note: None,
    };
    draft::metadata(header.build(), vec![draft::dispatch(None)])
}

/// Deterministic, poorly compressible attachment bytes.
fn payload(index: usize) -> Vec<u8> {
    support::pseudo_random(500 + index * 271)
}

/// A package with `count` described attachments.
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
    PackageSpec {
        timestamp: STAMP,
        ..PackageSpec::with_attachments(document(), attachments)
    }
}

#[track_caller]
fn write(spec: &PackageSpec) -> Vec<u8> {
    create::package(spec, &Limits::DEFAULT).expect("the package is written")
}

/// A package with `count` attachments, as the writer produces it.
fn source(count: usize) -> Vec<u8> {
    write(&spec_with(count))
}

/// Repack `image` under `edits`, returning the plan and the bytes.
#[track_caller]
fn repack(image: &[u8], edits: &Edits) -> (repack::RepackPlan, Vec<u8>) {
    let inventory = archive::inventory(image, &Limits::DEFAULT).expect("the image is an archive");
    let plan = repack::plan(
        &inventory,
        &Limits::DEFAULT,
        &MetadataLimits::DEFAULT,
        edits,
    )
    .expect("the package can be repacked");
    let bytes = repack::apply(&inventory, &plan, STAMP, &Limits::DEFAULT).expect("the bytes");
    (plan, bytes)
}

/// The entry names of an image, in central-directory order.
fn names(image: &[u8]) -> Vec<String> {
    let inventory = archive::inventory(image, &Limits::DEFAULT).expect("the image is an archive");
    inventory
        .entries()
        .iter()
        .map(|entry| String::from_utf8(entry.name_bytes().to_vec()).expect("UTF-8 names"))
        .collect()
}

/// The decoded bytes of every entry of an image, by name.
fn entry_bytes(image: &[u8], name: &str) -> Vec<u8> {
    let inventory = archive::inventory(image, &Limits::DEFAULT).expect("the image is an archive");
    let index = inventory
        .entries()
        .iter()
        .position(|entry| entry.name_bytes() == name.as_bytes())
        .unwrap_or_else(|| panic!("no entry named {name}"));
    inventory
        .entry_bytes(u32::try_from(index).expect("a small index"))
        .expect("the entry decodes")
}

/// The parsed document of an image.
fn parsed(image: &[u8]) -> Metadata {
    metadata::parse(
        &entry_bytes(image, create::METADATA_NAME),
        &MetadataLimits::DEFAULT,
    )
    .expect("the document parses")
}

// ------------------------------------------------------------- preservation

#[test]
fn repacking_with_no_edit_writes_the_package_it_was_given() {
    // The property the refusals exist to protect: an empty edit is the
    // identity, byte for byte, on a package this crate wrote.
    for count in [0, 1, 3] {
        let image = source(count);
        let (plan, bytes) = repack(&image, &Edits::default());
        assert_eq!(bytes, image, "{count} attachments");
        assert_eq!(plan.preserved().len(), count);
        assert!(plan.changed().is_empty());
        assert!(plan.added().is_empty());
        assert!(plan.removed().is_empty());
        assert!(plan.header_fields().is_empty());
    }
}

#[test]
fn a_dispatch_carrying_no_attachment_container_keeps_none() {
    // The identity has to hold for a document this crate did not write, and
    // the one shape its own writer never produced is a dispatch with no
    // `MELLEKLETEK` element at all — legal under M7, since the count is a
    // separate element, and possible only with nothing to list. The document
    // below is hand-authored for exactly that: everything the writer would
    // emit, minus the container.
    let authored = support::meta::Document {
        declared_count: Some("0".to_owned()),
        attachments: Vec::new(),
        handling_instructions: false,
        ..support::meta::Document::default()
    }
    .xml()
    .replace("<ns2:MELLEKLETEK></ns2:MELLEKLETEK>", "");
    assert!(
        !authored.contains("<ns2:MELLEKLETEK>"),
        "the document under test carries no container"
    );

    let image = Archive::of(vec![
        Entry::stored(create::MARKER_NAME.as_bytes(), create::MARKER_CONTENT),
        Entry::deflated(create::METADATA_NAME.as_bytes(), authored.as_bytes()),
    ])
    .build();
    let document = parsed(&image);
    assert!(!document.dispatches[0].attachments_present);

    let (plan, bytes) = repack(&image, &Edits::default());
    assert_eq!(plan.attachment_count(), 0);
    assert_eq!(
        entry_bytes(&bytes, create::METADATA_NAME),
        authored.as_bytes(),
        "an empty edit must not add the container the document did not carry"
    );

    // Adding an attachment gives the block a container, because the reference
    // it derives has to be listed inside one.
    let mut edits = Edits::default();
    edits.add.push(AttachmentAddition {
        file_name: "added.pdf".to_owned(),
        bytes: payload(9),
        description: Some("an added attachment".to_owned()),
    });
    let (_, grown) = repack(&image, &edits);
    let grown_document = parsed(&grown);
    assert!(grown_document.dispatches[0].attachments_present);
    assert_eq!(grown_document.attachment_count(), 1);
}

#[test]
fn repacking_is_idempotent() {
    let image = source(2);
    let (_, once) = repack(&image, &Edits::default());
    let (_, twice) = repack(&once, &Edits::default());
    assert_eq!(once, twice);
}

#[test]
fn every_preserved_attachment_keeps_its_bytes_exactly() {
    let image = source(3);
    let mut edits = Edits::default();
    edits.remove.push(1);
    edits.header.note = repack::OptionalEdit::Set("edited".to_owned());
    let (plan, bytes) = repack(&image, &edits);
    assert_eq!(plan.preserved(), [2, 3]);
    // Attachments 2 and 3 become 1 and 2, and their bytes are the input's.
    assert_eq!(
        entry_bytes(&bytes, "KRX/OCD/Payload/ID-1/melleklet-1.pdf"),
        entry_bytes(&image, "KRX/OCD/Payload/ID-2/melleklet-1.pdf")
    );
    assert_eq!(
        entry_bytes(&bytes, "KRX/OCD/Payload/ID-2/melleklet-2.pdf"),
        entry_bytes(&image, "KRX/OCD/Payload/ID-3/melleklet-2.pdf")
    );
}

#[test]
fn a_repacked_package_reads_back_with_no_failing_check() {
    let image = source(2);
    let mut edits = Edits::default();
    edits.add.push(AttachmentAddition::described(
        "added.pdf",
        payload(9),
        "the added attachment",
    ));
    let (_, bytes) = repack(&image, &edits);
    let report = create::verify_round_trip(&bytes, &Limits::DEFAULT, &MetadataLimits::DEFAULT)
        .expect("the package reads back");
    assert!(
        report
            .checks()
            .iter()
            .all(|check| !matches!(check.outcome, CheckOutcome::Fail(_))),
        "a repacked package must fail no structural check"
    );
}

// -------------------------------------------------------------- the edits

#[test]
fn setting_header_fields_changes_those_and_nothing_else() {
    let image = source(1);
    let before = parsed(&image);
    let mut edits = Edits::default();
    edits.header.consignment_id = Some("REPLACEMENT-ID".to_owned());
    edits.header.source_system = Some(SourceSystem::Posta);
    edits.header.note = repack::OptionalEdit::Set("added note".to_owned());
    let (plan, bytes) = repack(&image, &edits);
    assert_eq!(
        plan.header_fields(),
        [
            HeaderField::SourceSystem,
            HeaderField::ConsignmentId,
            HeaderField::Note
        ]
    );
    let after = parsed(&bytes);
    assert_eq!(after.header.consignment_id, "REPLACEMENT-ID");
    assert_eq!(after.header.source_system, SourceSystem::Posta);
    assert_eq!(after.header.note.as_deref(), Some("added note"));
    assert_eq!(after.header.version, before.header.version);
    assert_eq!(after.header.created_at_text, before.header.created_at_text);
    assert_eq!(
        after.header.consignment_kind,
        before.header.consignment_kind
    );
    assert_eq!(after.attachment_count(), before.attachment_count());
}

#[test]
fn clearing_an_optional_field_removes_the_element() {
    let image = source(0);
    let mut set = Edits::default();
    set.header.barcode = repack::OptionalEdit::Set("BARCODE-1".to_owned());
    let (_, with_barcode) = repack(&image, &set);
    assert_eq!(
        parsed(&with_barcode).header.barcode.as_deref(),
        Some("BARCODE-1")
    );

    let mut cleared = Edits::default();
    cleared.header.barcode = repack::OptionalEdit::Clear;
    let (plan, without) = repack(&with_barcode, &cleared);
    assert_eq!(plan.header_fields(), [HeaderField::Barcode]);
    assert_eq!(parsed(&without).header.barcode, None);
    assert_eq!(without, image, "clearing it restores the original package");
}

#[test]
fn adding_an_attachment_appends_it_and_derives_its_reference() {
    let image = source(1);
    let mut edits = Edits::default();
    edits.add.push(AttachmentAddition::described(
        "annex.pdf",
        b"annex bytes".to_vec(),
        "the annex",
    ));
    let (plan, bytes) = repack(&image, &edits);
    assert_eq!(plan.added(), [2]);
    assert_eq!(plan.entry_count(), 4);
    assert_eq!(
        names(&bytes).last().map(String::as_str),
        Some("KRX/OCD/Payload/ID-2/annex.pdf")
    );
    let document = parsed(&bytes);
    let added = document.attachments().nth(1).expect("the second reference");
    assert_eq!(added.number, 2);
    assert_eq!(added.file_name, "annex.pdf");
    assert_eq!(added.location, "KRX/OCD/Payload/ID-2");
    assert_eq!(added.description.as_deref(), Some("the annex"));
    assert_eq!(
        added.size_text, "1",
        "eleven bytes is one kilobyte, rounded up"
    );
    assert_eq!(
        entry_bytes(&bytes, "KRX/OCD/Payload/ID-2/annex.pdf"),
        b"annex bytes".to_vec()
    );
}

#[test]
fn replacing_an_attachment_changes_its_bytes_and_its_declared_size() {
    let image = source(2);
    let mut edits = Edits::default();
    edits.replace.push(AttachmentReplacement {
        number: 1,
        bytes: b"replacement".to_vec(),
    });
    let (plan, bytes) = repack(&image, &edits);
    assert_eq!(plan.changed(), [1]);
    assert_eq!(plan.preserved(), [2]);
    assert_eq!(
        entry_bytes(&bytes, "KRX/OCD/Payload/ID-1/melleklet-0.pdf"),
        b"replacement".to_vec()
    );
    let document = parsed(&bytes);
    let first = document.attachments().next().expect("the first reference");
    assert_eq!(first.file_name, "melleklet-0.pdf", "the name is preserved");
    assert_eq!(
        first.description.as_deref(),
        Some("árvíztűrő description 0"),
        "the description is preserved"
    );
    assert_eq!(first.size_text, "1", "MERET describes the new bytes");
}

#[test]
fn removing_every_attachment_leaves_a_document_declaring_none() {
    let image = source(2);
    let mut edits = Edits::default();
    edits.remove.extend([2, 1]);
    let (plan, bytes) = repack(&image, &edits);
    assert_eq!(plan.removed(), [1, 2], "the report is in number order");
    assert_eq!(plan.entry_count(), 2);
    assert_eq!(names(&bytes).len(), 2);
    let document = parsed(&bytes);
    assert_eq!(document.attachment_count(), 0);
    assert_eq!(
        document
            .dispatches
            .first()
            .and_then(|dispatch| dispatch.declared_attachment_count),
        Some(0)
    );
}

#[test]
fn removing_adding_and_replacing_together_renumber_the_result() {
    let image = source(3);
    let mut edits = Edits::default();
    edits.remove.push(2);
    edits.replace.push(AttachmentReplacement {
        number: 3,
        bytes: b"new third".to_vec(),
    });
    edits
        .add
        .push(AttachmentAddition::new("added.bin", b"added".to_vec()));
    let (plan, bytes) = repack(&image, &edits);
    assert_eq!(plan.preserved(), [1]);
    assert_eq!(plan.changed(), [3]);
    assert_eq!(plan.removed(), [2]);
    assert_eq!(plan.added(), [3]);
    assert_eq!(
        names(&bytes),
        [
            create::MARKER_NAME.to_owned(),
            create::METADATA_NAME.to_owned(),
            "KRX/OCD/Payload/ID-1/melleklet-0.pdf".to_owned(),
            "KRX/OCD/Payload/ID-2/melleklet-2.pdf".to_owned(),
            "KRX/OCD/Payload/ID-3/added.bin".to_owned(),
        ]
    );
}

// ---------------------------------------------------------------- the plan

#[test]
fn a_plan_is_a_pure_value_and_writing_it_twice_writes_the_same_bytes() {
    let image = source(2);
    let mut edits = Edits::default();
    edits.header.consignment_id = Some("STABLE".to_owned());
    let inventory = archive::inventory(&image, &Limits::DEFAULT).expect("the image is an archive");
    let plan = repack::plan(
        &inventory,
        &Limits::DEFAULT,
        &MetadataLimits::DEFAULT,
        &edits,
    )
    .expect("the plan");
    let first = repack::apply(&inventory, &plan, STAMP, &Limits::DEFAULT).expect("bytes");
    let second = repack::apply(&inventory, &plan, STAMP, &Limits::DEFAULT).expect("bytes");
    assert_eq!(first, second);
    assert_eq!(plan.attachment_count(), 2);
}

#[test]
fn a_repacked_package_equals_creating_the_same_package_from_scratch() {
    // The other half of the identity property: what repacking writes is what
    // `create` writes for the same request, so nothing about the result
    // depends on the package having been read rather than built.
    let image = source(1);
    let mut edits = Edits::default();
    edits.add.push(AttachmentAddition::described(
        "annex.pdf",
        b"annex bytes".to_vec(),
        "the annex",
    ));
    let (_, repacked) = repack(&image, &edits);

    let mut spec = spec_with(1);
    spec.attachments.push(AttachmentInput::described(
        "annex.pdf",
        b"annex bytes".to_vec(),
        "the annex",
    ));
    assert_eq!(repacked, write(&spec));
}

#[test]
fn every_header_field_can_be_set_and_is_reported_by_its_manifest_name() {
    // The names are what the command's report prints, so they are contract:
    // a caller reads `header_fields` to see what an edit touched.
    let image = source(0);
    let mut edits = Edits::default();
    edits.header.version = Some("1.0".to_owned());
    edits.header.source_system = Some(SourceSystem::Imap);
    edits.header.consignment_id = Some("ID-2".to_owned());
    edits.header.created_at_text = Some("2027-02-03T04:05:06".to_owned());
    edits.header.consignment_kind = Some(ConsignmentKind::Nyugta);
    edits.header.test = Some(false);
    edits.header.barcode = repack::OptionalEdit::Set("BARCODE".to_owned());
    edits.header.reference_id = repack::OptionalEdit::Set("REF".to_owned());
    edits.header.error_code = repack::OptionalEdit::Set("ERR".to_owned());
    edits.header.note = repack::OptionalEdit::Set("NOTE".to_owned());
    assert!(!edits.is_empty());

    let (plan, bytes) = repack(&image, &edits);
    let names: Vec<&str> = plan
        .header_fields()
        .iter()
        .map(|field| field.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "version",
            "source_system",
            "consignment_id",
            "created_at",
            "consignment_kind",
            "test",
            "barcode",
            "reference_id",
            "error_code",
            "note",
        ]
    );
    let header = parsed(&bytes).header;
    assert_eq!(header.version, "1.0");
    assert_eq!(header.source_system, SourceSystem::Imap);
    assert_eq!(header.consignment_id, "ID-2");
    assert_eq!(header.created_at_text, "2027-02-03T04:05:06");
    assert_eq!(header.consignment_kind, ConsignmentKind::Nyugta);
    assert!(!header.test);
    assert!(header.test_present, "setting TESZT writes the element");
    assert_eq!(header.barcode.as_deref(), Some("BARCODE"));
    assert_eq!(header.reference_id.as_deref(), Some("REF"));
    assert_eq!(header.error_code.as_deref(), Some("ERR"));
    assert_eq!(header.note.as_deref(), Some("NOTE"));
    assert!(Edits::default().is_empty());
}

#[test]
fn an_addition_without_a_description_reports_the_unresolved_rule_m11() {
    // Omitting `MELLEKLET_LEIRASA` is what one official example does, so it is
    // allowed and reported as undecided rather than refused.
    let image = source(0);
    let mut edits = Edits::default();
    edits
        .add
        .push(AttachmentAddition::new("plain.bin", b"x".to_vec()));
    let (plan, bytes) = repack(&image, &edits);
    assert_eq!(plan.added(), [1]);
    assert_eq!(plan.metadata().dispatches.len(), 1, "a block was placed");
    let report = create::verify_round_trip(&bytes, &Limits::DEFAULT, &MetadataLimits::DEFAULT)
        .expect("the package reads back");
    assert!(report.checks().iter().any(|check| matches!(
        check.outcome,
        CheckOutcome::Unresolved(rule) if rule.as_str() == "M11"
    )));
}
