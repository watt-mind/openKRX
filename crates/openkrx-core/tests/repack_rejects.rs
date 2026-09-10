//! Everything repacking refuses, and the code each refusal reports.
//!
//! The refusals are the feature: a package carrying something the writer
//! cannot re-emit is refused whole rather than repacked into one that quietly
//! lost part of it. Every archive here is built in this file by the test-only
//! synthetic writer, from values authored in this repository.

mod support;

use openkrx_core::create::{MARKER_CONTENT, MARKER_NAME, METADATA_NAME};
use openkrx_core::repack::{self, AttachmentReplacement, Edits};
use openkrx_core::{Limits, MetadataLimits, archive};
use support::meta::{Attachment, Document};
use support::{Archive, Entry};

/// The payload bytes every attachment entry in this file carries.
const PAYLOAD: &[u8] = b"synthetic payload bytes";

/// A canonical package: the marker, `document`, then one entry per payload.
///
/// The entry names are given whole so that a test can build a package that is
/// canonical in every respect but one.
fn package(document: &[u8], payloads: &[&str]) -> Vec<u8> {
    let mut entries = vec![
        Entry::stored(MARKER_NAME.as_bytes(), MARKER_CONTENT),
        Entry::deflated(METADATA_NAME.as_bytes(), document),
    ];
    for name in payloads {
        entries.push(Entry::stored(name.as_bytes(), PAYLOAD));
    }
    Archive::of(entries).build()
}

/// A document declaring `count` attachments the canonical layout would hold.
fn document(count: usize) -> Document {
    let attachments = (0..count)
        .map(|index| {
            let number = i64::try_from(index + 1).expect("a small number");
            Attachment {
                // `MERET` as the writer derives it: kilobytes, rounded up.
                size: "1".to_owned(),
                ..Attachment::new(
                    number,
                    &format!("payload-{number}.bin"),
                    &format!("KRX/OCD/Payload/ID-{number}"),
                )
            }
        })
        .collect::<Vec<_>>();
    Document {
        declared_count: Some(count.to_string()),
        attachments,
        handling_instructions: false,
        ..Document::default()
    }
}

/// The payload entry names of a canonical package with `count` attachments.
fn payload_names(count: usize) -> Vec<String> {
    (1..=count)
        .map(|number| format!("KRX/OCD/Payload/ID-{number}/payload-{number}.bin"))
        .collect()
}

/// A canonical package with `count` attachments, refused by nothing.
fn canonical(count: usize) -> Vec<u8> {
    let names = payload_names(count);
    package(
        &document(count).bytes(),
        &names.iter().map(String::as_str).collect::<Vec<_>>(),
    )
}

/// The code repacking `image` under `edits` reports, or a panic on success.
#[track_caller]
fn refusal(image: &[u8], edits: &Edits) -> &'static str {
    let inventory = archive::inventory(image, &Limits::DEFAULT).expect("the image is an archive");
    repack::plan(
        &inventory,
        &Limits::DEFAULT,
        &MetadataLimits::DEFAULT,
        edits,
    )
    .expect_err("the package must be refused")
    .code()
}

/// The same, with no edit at all.
#[track_caller]
fn refused(image: &[u8]) -> &'static str {
    refusal(image, &Edits::default())
}

#[test]
fn a_canonical_package_is_accepted_so_the_refusals_below_are_about_one_thing() {
    // The control: everything the other tests change is a single deviation
    // from this package, so a refusal below cannot be an artefact of the
    // builder.
    let image = canonical(2);
    let inventory = archive::inventory(&image, &Limits::DEFAULT).expect("an archive");
    let plan = repack::plan(
        &inventory,
        &Limits::DEFAULT,
        &MetadataLimits::DEFAULT,
        &Edits::default(),
    )
    .expect("the canonical package is repackable");
    assert_eq!(plan.preserved(), [1, 2]);
}

// ------------------------------------------------------------- the layout

#[test]
fn a_metadata_document_under_another_root_prefix_is_refused() {
    // The layout rule A19 leaves open. Repacking would move every entry of
    // the package, so it is refused rather than silently relaid out.
    let names = ["Payload/ID-1/payload-1.bin"];
    let document = Document {
        attachments: vec![Attachment {
            size: "1".to_owned(),
            ..Attachment::new(1, "payload-1.bin", "Payload/ID-1")
        }],
        declared_count: Some("1".to_owned()),
        handling_instructions: false,
        ..Document::default()
    };
    let image = Archive::of(vec![
        Entry::stored(b"mimetype", MARKER_CONTENT),
        Entry::deflated(b"Metalayer/KULDEMENY_META.xml", &document.bytes()),
        Entry::stored(names[0].as_bytes(), PAYLOAD),
    ])
    .build();
    assert_eq!(refused(&image), "repack.unsupported.root_prefix");
}

#[test]
fn a_metadata_file_name_spelled_differently_is_refused() {
    // Rule M12 leaves the casing open; re-emitting the document would rename
    // the entry.
    let image = Archive::of(vec![
        Entry::stored(MARKER_NAME.as_bytes(), MARKER_CONTENT),
        Entry::deflated(
            b"KRX/OCD/Metalayer/kuldemeny_meta.xml",
            &document(0).bytes(),
        ),
    ])
    .build();
    assert_eq!(refused(&image), "repack.unsupported.metadata_name");
}

#[test]
fn an_archive_with_no_metadata_document_is_refused() {
    let image = Archive::of(vec![Entry::stored(MARKER_NAME.as_bytes(), MARKER_CONTENT)]).build();
    assert_eq!(refused(&image), "repack.unsupported.metadata_missing");
}

#[test]
fn an_archive_with_two_metadata_documents_is_refused() {
    // Nothing settles which of the two would be edited, so neither is.
    let image = Archive::of(vec![
        Entry::stored(MARKER_NAME.as_bytes(), MARKER_CONTENT),
        Entry::deflated(METADATA_NAME.as_bytes(), &document(0).bytes()),
        Entry::deflated(
            b"KRX/OCD/other/Metalayer/KULDEMENY_META.xml",
            &document(0).bytes(),
        ),
    ])
    .build();
    assert_eq!(refused(&image), "repack.unsupported.metadata_missing");
}

#[test]
fn a_marker_that_is_missing_elsewhere_or_holds_something_else_is_refused() {
    let document = document(0).bytes();
    let missing = Archive::of(vec![Entry::deflated(METADATA_NAME.as_bytes(), &document)]).build();
    assert_eq!(refused(&missing), "repack.unsupported.marker");

    let second = Archive::of(vec![
        Entry::deflated(METADATA_NAME.as_bytes(), &document),
        Entry::stored(MARKER_NAME.as_bytes(), MARKER_CONTENT),
    ])
    .build();
    assert_eq!(refused(&second), "repack.unsupported.marker");

    let wrong = Archive::of(vec![
        Entry::stored(MARKER_NAME.as_bytes(), b"application/zip"),
        Entry::deflated(METADATA_NAME.as_bytes(), &document),
    ])
    .build();
    assert_eq!(refused(&wrong), "repack.unsupported.marker");
}

#[test]
fn an_entry_the_documented_layout_does_not_place_is_refused() {
    // `signatures.xml` (A7) and a service-specific document (A11–A16) reach
    // this: the writer emits neither, so repacking would drop them.
    for extra in [
        "KRX/OCD/signatures.xml",
        "KRX/OCD/Payload/ID-1/nested/deep.bin",
        "KRX/OCD/Payload/ID1/payload-1.bin",
        "KRX/OCD/Payload/ID-2/payload-2.bin",
    ] {
        let image = package(&document(0).bytes(), &[extra]);
        assert_eq!(refused(&image), "repack.unsupported.extra_entry", "{extra}");
    }
}

// ----------------------------------------------------------- the document

#[test]
fn a_document_carrying_elements_outside_the_grammar_is_refused() {
    // The reader counts them (A9) and keeps none, so the writer cannot put
    // them back.
    let document = Document {
        extra_body: "<ns2:SAJAT_ELEM>value</ns2:SAJAT_ELEM>".to_owned(),
        ..document(0)
    };
    let image = package(&document.bytes(), &[]);
    assert_eq!(refused(&image), "repack.unsupported.unknown_elements");
}

#[test]
fn a_block_the_reader_records_only_the_presence_of_is_refused() {
    // `ERKEZTETES`, `BONTASOK` and `TERTIVEVENY` would be written back empty.
    for element in ["ERKEZTETES", "BONTASOK"] {
        let document = Document {
            extra_body: format!("<ns2:{element}><ns2:X>text</ns2:X></ns2:{element}>"),
            ..document(0)
        };
        let image = package(&document.bytes(), &[]);
        assert_eq!(
            refused(&image),
            "repack.unsupported.opaque_block",
            "{element}"
        );
    }
    // `TERTIVEVENY` closes the M2 sequence, so it is appended rather than
    // inserted after the header.
    let xml = document(0).xml().replace(
        "</ns2:KULDEMENY>",
        "<ns2:TERTIVEVENY><ns2:X>text</ns2:X></ns2:TERTIVEVENY></ns2:KULDEMENY>",
    );
    assert_eq!(
        refused(&package(xml.as_bytes(), &[])),
        "repack.unsupported.opaque_block"
    );
}

#[test]
fn an_unqualified_handling_instruction_element_is_refused() {
    // M8's one unqualified element: its content is not retained either.
    let document = Document {
        handling_instructions: true,
        ..document(0)
    };
    let image = package(&document.bytes(), &[]);
    assert_eq!(refused(&image), "repack.unsupported.opaque_block");
}

#[test]
fn a_document_with_two_dispatch_blocks_is_refused() {
    // Hand-written, because the synthetic document builder emits one block.
    // The header-only document carries no `EXPEDIALASOK` of its own, so the
    // two blocks below are the whole dispatch section.
    let two = "<ns2:EXPEDIALASOK><ns2:EXPEDIALAS><ns2:MELLEKLETEK></ns2:MELLEKLETEK>\
</ns2:EXPEDIALAS><ns2:EXPEDIALAS><ns2:MELLEKLETEK></ns2:MELLEKLETEK>\
</ns2:EXPEDIALAS></ns2:EXPEDIALASOK>";
    let xml = Document::header_only()
        .xml()
        .replace("</ns2:KULDEMENY>", &format!("{two}</ns2:KULDEMENY>"));
    let image = package(xml.as_bytes(), &[]);
    assert_eq!(refused(&image), "repack.unsupported.dispatch_count");
}

#[test]
fn a_reference_the_writer_would_derive_differently_is_refused() {
    // `MERET` in some other unit — rule M13 leaves the unit open — would be
    // rewritten by the writer, so the package is refused instead.
    let mut document = document(1);
    document.attachments[0].size = "12.5".to_owned();
    let names = payload_names(1);
    let image = package(
        &document.bytes(),
        &names.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    assert_eq!(refused(&image), "repack.unsupported.attachment_reference");
}

#[test]
fn a_declared_count_that_disagrees_with_the_references_is_refused() {
    let mut document = document(1);
    document.declared_count = Some("7".to_owned());
    let names = payload_names(1);
    let image = package(
        &document.bytes(),
        &names.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    assert_eq!(refused(&image), "repack.unsupported.attachment_reference");
}

#[test]
fn a_dispatch_that_declares_no_count_at_all_is_refused() {
    // The writer derives `MELLEKLETEK_SZAMA` and always emits it, while the
    // reader records an absent element as `None` (M7). Repacking a document
    // that omitted it would therefore *add* it — a change nobody asked for —
    // so the absence is refused exactly as a wrong count is. Without this the
    // empty edit would not be the identity it is documented to be, which the
    // second half of this test asserts directly.
    let mut document = document(1);
    document.declared_count = None;
    let names = payload_names(1);
    let image = package(
        &document.bytes(),
        &names.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    assert_eq!(refused(&image), "repack.unsupported.attachment_reference");

    // The same document with the element present is accepted, so the refusal
    // above is about the count alone and not about the rest of the package.
    let accepted = canonical(1);
    let inventory = archive::inventory(&accepted, &Limits::DEFAULT).expect("an archive");
    let plan = repack::plan(
        &inventory,
        &Limits::DEFAULT,
        &MetadataLimits::DEFAULT,
        &Edits::default(),
    )
    .expect("a declared count that agrees is repackable");
    let bytes = repack::apply(
        &inventory,
        &plan,
        openkrx_core::create::FixedTimestamp::EPOCH,
        &Limits::DEFAULT,
    )
    .expect("bytes");
    let rewritten = archive::inventory(&bytes, &Limits::DEFAULT).expect("an archive");
    assert_eq!(
        rewritten.entry_bytes(1).expect("the document"),
        inventory.entry_bytes(1).expect("the document"),
        "an empty edit must not add or drop a single element"
    );
}

#[test]
fn a_reference_naming_another_entry_is_refused() {
    let mut document = document(1);
    document.attachments[0].file_name = "another-name.bin".to_owned();
    let names = payload_names(1);
    let image = package(
        &document.bytes(),
        &names.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    assert_eq!(refused(&image), "repack.unsupported.attachment_entry");
}

#[test]
fn a_payload_entry_no_reference_declares_is_refused() {
    let names = payload_names(1);
    let image = package(
        &document(0).bytes(),
        &names.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    assert_eq!(refused(&image), "repack.unsupported.attachment_entry");
}

#[test]
fn a_document_that_does_not_parse_reports_the_parser_s_own_code() {
    let image = package(b"<not-a-document/>", &[]);
    assert_eq!(refused(&image), "metadata.unsupported.namespace");
}

// -------------------------------------------------------------- the edits

#[test]
fn an_edit_naming_an_attachment_the_package_does_not_hold_is_refused() {
    let image = canonical(2);
    for number in [0, 3, 99] {
        let mut edits = Edits::default();
        edits.remove.push(number);
        assert_eq!(
            refusal(&image, &edits),
            "repack.invalid.no_such_attachment",
            "removing {number}"
        );
        let mut edits = Edits::default();
        edits.replace.push(AttachmentReplacement {
            number,
            bytes: b"x".to_vec(),
        });
        assert_eq!(
            refusal(&image, &edits),
            "repack.invalid.no_such_attachment",
            "replacing {number}"
        );
    }
}

#[test]
fn two_edits_naming_the_same_attachment_are_refused() {
    // What the caller wants done to it is not decided by the request, so
    // openKRX asks rather than choosing.
    let image = canonical(2);
    let mut removed_twice = Edits::default();
    removed_twice.remove.extend([1, 1]);
    assert_eq!(
        refusal(&image, &removed_twice),
        "repack.invalid.duplicate_target"
    );

    let mut removed_and_replaced = Edits::default();
    removed_and_replaced.remove.push(2);
    removed_and_replaced.replace.push(AttachmentReplacement {
        number: 2,
        bytes: b"x".to_vec(),
    });
    assert_eq!(
        refusal(&image, &removed_and_replaced),
        "repack.invalid.duplicate_target"
    );
}

#[test]
fn a_refusal_names_the_attachment_number_and_carries_nothing_else() {
    let image = canonical(1);
    let mut edits = Edits::default();
    edits.remove.push(4);
    let inventory = archive::inventory(&image, &Limits::DEFAULT).expect("an archive");
    let error = repack::plan(
        &inventory,
        &Limits::DEFAULT,
        &MetadataLimits::DEFAULT,
        &edits,
    )
    .expect_err("refused");
    assert_eq!(error.attachment_number(), Some(4));
    assert_eq!(error.entry_index(), None);
    assert_eq!(
        error.to_string(),
        "repack.invalid.no_such_attachment at attachment number 4"
    );
}

// ---------------------------------------------------------------- the plan

#[test]
fn applying_a_plan_to_another_package_is_refused_rather_than_written() {
    // The bytes a plan preserves are read from the inventory it was made
    // from; another one would silently write different content.
    let first = canonical(2);
    let second = canonical(1);
    let inventory = archive::inventory(&first, &Limits::DEFAULT).expect("an archive");
    let plan = repack::plan(
        &inventory,
        &Limits::DEFAULT,
        &MetadataLimits::DEFAULT,
        &Edits::default(),
    )
    .expect("a plan");
    let other = archive::inventory(&second, &Limits::DEFAULT).expect("an archive");
    let error = repack::apply(
        &other,
        &plan,
        openkrx_core::create::FixedTimestamp::EPOCH,
        &Limits::DEFAULT,
    )
    .expect_err("refused");
    assert_eq!(error.code(), "repack.invalid.inventory_mismatch");
}

#[test]
fn a_result_above_the_entry_ceiling_is_refused_while_planning() {
    let image = canonical(1);
    let mut limits = Limits::DEFAULT;
    limits.max_entries = 3;
    let mut edits = Edits::default();
    edits.add.push(repack::AttachmentAddition::new(
        "added.bin",
        b"added".to_vec(),
    ));
    let inventory = archive::inventory(&image, &Limits::DEFAULT).expect("an archive");
    let error =
        repack::plan(&inventory, &limits, &MetadataLimits::DEFAULT, &edits).expect_err("refused");
    assert_eq!(error.code(), "create.over_limit.entries");
}

#[test]
fn a_pass_through_failure_keeps_the_reporting_of_the_layer_it_came_from() {
    // Repacking composes three layers, and each one's diagnostic reaches a
    // caller unchanged: the code, the entry index and the numbers are the
    // ones that layer produces, never a repack.* code standing in for them.
    let image = canonical(1);
    let mut limits = Limits::DEFAULT;
    limits.max_entries = 1;
    let refused = archive::inventory(&image, &limits);
    assert!(refused.is_err(), "the reader itself refuses this one");

    let inventory = archive::inventory(&image, &Limits::DEFAULT).expect("an archive");
    let mut edits = Edits::default();
    edits
        .add
        .push(repack::AttachmentAddition::new("added.bin", vec![0_u8; 32]));
    let mut tight = Limits::DEFAULT;
    tight.max_entry_decoded_bytes = 8;
    let error =
        repack::plan(&inventory, &tight, &MetadataLimits::DEFAULT, &edits).expect_err("refused");
    assert_eq!(error.code(), "create.over_limit.entry_bytes");
    assert_eq!(error.attachment_number(), None);
    assert!(
        error.to_string().contains("limit 8"),
        "the writer's own numbers survive: {error}"
    );

    let unparsable = package(b"<not-a-document/>", &[]);
    let inventory = archive::inventory(&unparsable, &Limits::DEFAULT).expect("an archive");
    let error = repack::plan(
        &inventory,
        &Limits::DEFAULT,
        &MetadataLimits::DEFAULT,
        &Edits::default(),
    )
    .expect_err("refused");
    assert_eq!(error.to_string(), "metadata.unsupported.namespace");
    assert_eq!(error.entry_index(), None);
}

#[test]
fn an_unsupported_refusal_prints_the_entry_it_concerns_and_nothing_else() {
    let image = package(&document(0).bytes(), &["KRX/OCD/signatures.xml"]);
    let inventory = archive::inventory(&image, &Limits::DEFAULT).expect("an archive");
    let error = repack::plan(
        &inventory,
        &Limits::DEFAULT,
        &MetadataLimits::DEFAULT,
        &Edits::default(),
    )
    .expect_err("refused");
    assert_eq!(
        error.to_string(),
        "repack.unsupported.extra_entry at entry 2",
        "a diagnostic carries the code and the index, never the entry name"
    );
}
