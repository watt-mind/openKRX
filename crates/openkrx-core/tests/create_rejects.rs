//! What the writer refuses, and the stable code each refusal reports.
//!
//! Every documented `create.*` code is produced by a test here or in
//! `create_package.rs`, and every one of them refuses the whole package: a
//! caller never receives a quietly reduced one.

mod support;

use openkrx_core::create::{
    self, AttachmentInput, CreateError, FixedTimestamp, InvalidKind, PackageSpec, UnsafeNameKind,
};
use openkrx_core::metadata::{Metadata, TARGET_NAMESPACE};
use openkrx_core::{Limits, MetadataLimits, archive, metadata};
use support::meta::{Attachment, Document};

#[track_caller]
fn parse(document: &[u8]) -> Metadata {
    metadata::parse(document, &MetadataLimits::DEFAULT).expect("the document parses")
}

/// A header and one empty dispatch block: the shape a caller starts from.
fn metadata_with_dispatch() -> Metadata {
    parse(
        &Document {
            declared_count: None,
            attachments: Vec::new(),
            handling_instructions: true,
            ..Document::default()
        }
        .bytes(),
    )
}

/// One attachment of `bytes` bytes named `file_name`.
fn spec_named(file_name: &str) -> PackageSpec {
    PackageSpec::with_attachments(
        metadata_with_dispatch(),
        vec![AttachmentInput::new(file_name, b"payload".to_vec())],
    )
}

/// The code writing `spec` under `limits` reports.
#[track_caller]
fn refuses(spec: &PackageSpec, limits: &Limits) -> CreateError {
    create::package(spec, limits).expect_err("the package is refused")
}

#[track_caller]
fn accepts(spec: &PackageSpec, limits: &Limits) -> Vec<u8> {
    create::package(spec, limits).expect("the package is written")
}

// ------------------------------------------------------- names this refuses

#[test]
fn every_unsafe_file_name_class_is_refused_with_its_own_code() {
    for (file_name, kind, code) in [
        ("", UnsafeNameKind::Empty, "create.unsafe_name.empty"),
        (
            "a/b.pdf",
            UnsafeNameKind::Separator,
            "create.unsafe_name.separator",
        ),
        (
            "a\\b.pdf",
            UnsafeNameKind::Separator,
            "create.unsafe_name.separator",
        ),
        (
            ".",
            UnsafeNameKind::CurrentComponent,
            "create.unsafe_name.current_component",
        ),
        (
            "..",
            UnsafeNameKind::ParentComponent,
            "create.unsafe_name.parent_component",
        ),
        (
            "a\u{1}b",
            UnsafeNameKind::ControlCharacter,
            "create.unsafe_name.control_character",
        ),
        (
            "a\u{85}b",
            UnsafeNameKind::ControlCharacter,
            "create.unsafe_name.control_character",
        ),
        (
            "report.",
            UnsafeNameKind::TrailingDot,
            "create.unsafe_name.trailing_dot",
        ),
        (
            "report ",
            UnsafeNameKind::TrailingSpace,
            "create.unsafe_name.trailing_space",
        ),
        (
            " report",
            UnsafeNameKind::LeadingSpace,
            "create.unsafe_name.leading_space",
        ),
        (
            "con.txt",
            UnsafeNameKind::ReservedDeviceName,
            "create.unsafe_name.reserved_device_name",
        ),
        (
            "a:b.pdf",
            UnsafeNameKind::ReservedColon,
            "create.unsafe_name.colon",
        ),
        (
            "a*b.pdf",
            UnsafeNameKind::ReservedCharacter,
            "create.unsafe_name.reserved_character",
        ),
    ] {
        let error = refuses(&spec_named(file_name), &Limits::DEFAULT);
        assert_eq!(
            error,
            CreateError::UnsafeName {
                kind,
                index: Some(0)
            },
            "{file_name:?}"
        );
        assert_eq!(error.code(), code, "{file_name:?}");
        assert_eq!(error.attachment_index(), Some(0));
        assert_eq!(
            error.to_string(),
            format!("{code} at attachment 0"),
            "a diagnostic never carries the name itself"
        );
    }
}

#[test]
fn a_name_the_extraction_planner_would_refuse_is_refused_at_creation() {
    // The archive layer would accept `CON`; the planner would not, and a
    // package openKRX writes must be one openKRX can extract on any platform.
    let spec = spec_named("CON");
    assert!(create::package(&spec, &Limits::DEFAULT).is_err());
    let image = accepts(&spec_named("CONSOLE"), &Limits::DEFAULT);
    let inventory = archive::inventory(&image, &Limits::DEFAULT).expect("the image is an archive");
    openkrx_core::extract::plan(&inventory, &openkrx_core::extract::ExtractLimits::DEFAULT)
        .expect("what was written can be extracted");
}

#[test]
fn a_name_that_merely_looks_unusual_is_written() {
    for file_name in ["árvíztűrő.pdf", "a b.pdf", ".hidden", "COM0", "a.b.c"] {
        accepts(&spec_named(file_name), &Limits::DEFAULT);
    }
}

// ------------------------------------------------- requests that contradict

#[test]
fn a_supplied_reference_that_disagrees_with_the_attachments_is_refused() {
    // The document describes one 12.5-unit attachment called `synthetic.pdf`;
    // the request carries different bytes under a different name.
    let metadata = parse(&Document::default().bytes());
    let spec = PackageSpec::with_attachments(
        metadata,
        vec![AttachmentInput::new("other.pdf", b"payload".to_vec())],
    );
    let error = refuses(&spec, &Limits::DEFAULT);
    assert_eq!(error.code(), "create.invalid.reference_mismatch");
    assert_eq!(error.attachment_index(), Some(0));
}

#[test]
fn a_declared_count_that_disagrees_with_the_attachments_is_refused() {
    let metadata = parse(
        &Document {
            declared_count: Some("2".to_owned()),
            attachments: Vec::new(),
            ..Document::default()
        }
        .bytes(),
    );
    let spec = PackageSpec::with_attachments(
        metadata,
        vec![AttachmentInput::new("a.pdf", b"payload".to_vec())],
    );
    let error = refuses(&spec, &Limits::DEFAULT);
    assert_eq!(error.code(), "create.invalid.reference_mismatch");
    assert_eq!(error.attachment_index(), None);
    assert_eq!(error.to_string(), "create.invalid.reference_mismatch");
}

#[test]
fn a_reference_list_of_the_wrong_length_is_refused() {
    let metadata = parse(
        &Document {
            declared_count: None,
            attachments: vec![
                Attachment::new(1, "a.pdf", "KRX/OCD/Payload/ID-1"),
                Attachment::new(2, "b.pdf", "KRX/OCD/Payload/ID-2"),
            ],
            ..Document::default()
        }
        .bytes(),
    );
    let spec = PackageSpec::with_attachments(
        metadata,
        vec![AttachmentInput::new("a.pdf", b"payload".to_vec())],
    );
    assert_eq!(
        refuses(&spec, &Limits::DEFAULT).code(),
        "create.invalid.reference_mismatch"
    );
}

#[test]
fn a_reference_the_writer_would_have_derived_is_accepted() {
    // The one caller-supplied shape that agrees: written once, read back, and
    // handed to the writer again. `create_package.rs` holds the byte equality.
    let first = accepts(
        &PackageSpec::with_attachments(
            metadata_with_dispatch(),
            vec![AttachmentInput::new("a.pdf", b"payload".to_vec())],
        ),
        &Limits::DEFAULT,
    );
    let inventory = archive::inventory(&first, &Limits::DEFAULT).expect("the image is an archive");
    let document = parse(&inventory.entry_bytes(1).expect("the document decodes"));
    accepts(
        &PackageSpec::with_attachments(
            document,
            vec![AttachmentInput::new("a.pdf", b"payload".to_vec())],
        ),
        &Limits::DEFAULT,
    );
}

#[test]
fn attachments_with_nowhere_to_list_them_are_refused() {
    let spec = PackageSpec::with_attachments(
        parse(&Document::header_only().bytes()),
        vec![AttachmentInput::new("a.pdf", b"payload".to_vec())],
    );
    let error = refuses(&spec, &Limits::DEFAULT);
    assert_eq!(error.code(), "create.invalid.dispatch_count");
    assert_eq!(error.attachment_index(), None);
}

#[test]
fn a_document_with_two_dispatch_blocks_is_refused() {
    let document = format!(
        "<ns2:KULDEMENY xmlns:ns2=\"{TARGET_NAMESPACE}\">\
<ns2:FEJRESZ>\
<ns2:KRX_VERZIOSZAM>v0.9</ns2:KRX_VERZIOSZAM>\
<ns2:FORRASRENDSZER_AZONOSITO>KER</ns2:FORRASRENDSZER_AZONOSITO>\
<ns2:KULDEMENY_AZONOSITO>AZ-1</ns2:KULDEMENY_AZONOSITO>\
<ns2:KULDEMENY_LETREHOZASANAK_IDEJE>2026-01-02T03:04:05.000+01:00\
</ns2:KULDEMENY_LETREHOZASANAK_IDEJE>\
<ns2:KULDEMENY_TIPUS>KULDEMENY</ns2:KULDEMENY_TIPUS>\
<ns2:TESZT>false</ns2:TESZT>\
</ns2:FEJRESZ>\
<ns2:EXPEDIALASOK><ns2:EXPEDIALAS></ns2:EXPEDIALAS>\
<ns2:EXPEDIALAS></ns2:EXPEDIALAS></ns2:EXPEDIALASOK>\
</ns2:KULDEMENY>"
    );
    let metadata = parse(document.as_bytes());
    assert_eq!(metadata.dispatches.len(), 2);
    assert_eq!(
        refuses(&PackageSpec::new(metadata), &Limits::DEFAULT).code(),
        "create.invalid.dispatch_count"
    );
}

#[test]
fn a_document_carrying_elements_the_grammar_does_not_define_is_refused() {
    // A9 lets a container carry further information and the reader counts it;
    // the writer emits the grammar alone, so it refuses rather than drop it.
    let metadata = parse(
        &Document {
            extra_body: "<ns2:ISMERETLEN>x</ns2:ISMERETLEN>".to_owned(),
            ..Document::default()
        }
        .bytes(),
    );
    assert_eq!(metadata.unknown_elements, 1);
    let error = refuses(&PackageSpec::new(metadata), &Limits::DEFAULT);
    assert_eq!(error.code(), "create.invalid.unknown_elements");
}

// ------------------------------------------------------- text this refuses

#[test]
fn a_character_xml_cannot_carry_is_refused_rather_than_dropped() {
    for text in [
        "bad\u{1}text",
        "bad\u{c}text",
        "carriage\rreturn",
        "non\u{fffe}character",
    ] {
        let spec = PackageSpec::with_attachments(
            metadata_with_dispatch(),
            vec![AttachmentInput::described(
                "a.pdf",
                b"payload".to_vec(),
                text,
            )],
        );
        let error = refuses(&spec, &Limits::DEFAULT);
        assert_eq!(error.code(), "create.invalid.text", "{text:?}");
        assert_eq!(error.attachment_index(), Some(0));
    }
}

#[test]
fn text_the_reader_would_trim_is_refused_rather_than_silently_changed() {
    for text in [" leading", "trailing ", "\nnewline\n"] {
        let spec = PackageSpec::with_attachments(
            metadata_with_dispatch(),
            vec![AttachmentInput::described(
                "a.pdf",
                b"payload".to_vec(),
                text,
            )],
        );
        assert_eq!(
            refuses(&spec, &Limits::DEFAULT).code(),
            "create.invalid.untrimmed_text",
            "{text:?}"
        );
    }
}

#[test]
fn text_xml_can_carry_is_written_and_read_back_unchanged() {
    let text = "tab\there\nand & < > árvíztűrő";
    let spec = PackageSpec::with_attachments(
        metadata_with_dispatch(),
        vec![AttachmentInput::described(
            "a.pdf",
            b"payload".to_vec(),
            text,
        )],
    );
    let image = accepts(&spec, &Limits::DEFAULT);
    let inventory = archive::inventory(&image, &Limits::DEFAULT).expect("the image is an archive");
    let document = parse(&inventory.entry_bytes(1).expect("the document decodes"));
    assert_eq!(
        document
            .attachments()
            .next()
            .expect("one reference")
            .description
            .as_deref(),
        Some(text)
    );
}

#[test]
fn a_timestamp_the_dos_fields_cannot_express_is_refused() {
    for (year, month, day, hour, minute, second) in [
        (1979, 12, 31, 23, 59, 58),
        (2108, 1, 1, 0, 0, 0),
        (2026, 13, 1, 0, 0, 0),
        (2026, 0, 1, 0, 0, 0),
        (2026, 2, 30, 0, 0, 0),
        (2026, 1, 0, 0, 0, 0),
        (2026, 1, 1, 24, 0, 0),
        (2026, 1, 1, 0, 60, 0),
        (2026, 1, 1, 0, 0, 60),
    ] {
        let error = FixedTimestamp::from_parts(year, month, day, hour, minute, second)
            .expect_err("the value is refused");
        assert_eq!(error.code(), "create.invalid.timestamp");
        assert_eq!(
            error,
            CreateError::Invalid {
                kind: InvalidKind::Timestamp,
                index: None
            }
        );
    }
    // The boundary values themselves are accepted, leap day included.
    for (year, month, day) in [(1980, 1, 1), (2107, 12, 31), (2024, 2, 29)] {
        FixedTimestamp::from_parts(year, month, day, 23, 59, 59).expect("a valid time");
    }
}

// ------------------------------------------------------ limits on the output

/// A package of `count` attachments of `bytes` bytes each.
fn spec_sized(count: usize, bytes: usize) -> PackageSpec {
    let attachments = (0..count)
        .map(|index| AttachmentInput::new(format!("a{index}.pdf"), vec![b'x'; bytes]))
        .collect();
    PackageSpec::with_attachments(metadata_with_dispatch(), attachments)
}

#[test]
fn the_entry_count_limit_holds_at_its_boundary() {
    let mut limits = Limits::DEFAULT;
    limits.max_entries = 3;
    accepts(&spec_sized(1, 16), &limits);
    let error = refuses(&spec_sized(2, 16), &limits);
    assert_eq!(error.code(), "create.over_limit.entries");
    assert_eq!(
        error.to_string(),
        "create.over_limit.entries (limit 3, observed 4)"
    );
}

#[test]
fn the_name_length_limit_holds_at_its_boundary() {
    // `KRX/OCD/Payload/ID-1/` is 21 bytes, and the metadata document's own name
    // is 36, so 36 is the smallest ceiling that can write anything at all.
    let mut limits = Limits::DEFAULT;
    limits.max_name_bytes = 36;
    let fits = "x".repeat(15);
    accepts(&spec_named(&fits), &limits);
    let error = refuses(&spec_named(&format!("{fits}x")), &limits);
    assert_eq!(error.code(), "create.over_limit.name_bytes");
    assert_eq!(
        error.to_string(),
        "create.over_limit.name_bytes (limit 36, observed 37) at attachment 0"
    );
    // The metadata document's own name is checked on the same path.
    limits.max_name_bytes = 35;
    let error = refuses(&spec_named("a.pdf"), &limits);
    assert_eq!(error.code(), "create.over_limit.name_bytes");
    assert_eq!(error.attachment_index(), None);
}

#[test]
fn the_entry_size_limit_holds_at_its_boundary() {
    let mut limits = Limits::DEFAULT;
    limits.max_entry_decoded_bytes = 4096;
    accepts(&spec_sized(1, 4096), &limits);
    let error = refuses(&spec_sized(1, 4097), &limits);
    assert_eq!(error.code(), "create.over_limit.entry_bytes");
    assert_eq!(error.attachment_index(), Some(0));
}

#[test]
fn the_total_size_limit_holds_at_its_boundary() {
    let spec = spec_sized(2, 1024);
    let image = accepts(&spec, &Limits::DEFAULT);
    let inventory = archive::inventory(&image, &Limits::DEFAULT).expect("the image is an archive");
    let total = inventory.total_decoded_bytes();
    let mut limits = Limits::DEFAULT;
    limits.max_total_decoded_bytes = total;
    accepts(&spec, &limits);
    limits.max_total_decoded_bytes = total - 1;
    let error = refuses(&spec, &limits);
    assert_eq!(error.code(), "create.over_limit.total_bytes");
    assert_eq!(error.attachment_index(), Some(1));
}

#[test]
fn the_archive_size_limit_holds_at_its_boundary() {
    let spec = spec_sized(1, 4096);
    let image = accepts(&spec, &Limits::DEFAULT);
    let mut limits = Limits::DEFAULT;
    limits.max_archive_bytes = image.len() as u64;
    accepts(&spec, &limits);
    limits.max_archive_bytes = image.len() as u64 - 1;
    let error = refuses(&spec, &limits);
    assert_eq!(error.code(), "create.over_limit.archive_bytes");
    assert_eq!(
        error.to_string(),
        format!(
            "create.over_limit.archive_bytes (limit {}, observed {})",
            image.len() - 1,
            image.len()
        )
    );
}

#[test]
fn a_package_written_at_a_tightened_ceiling_reads_back_at_the_same_one() {
    // The point of enforcing the reader's limits on the way out: whatever the
    // configuration, what was written is readable under it.
    let spec = spec_sized(3, 2048);
    let mut limits = Limits::DEFAULT;
    limits.max_entries = 5;
    limits.max_entry_decoded_bytes = 2048;
    let image = accepts(&spec, &limits);
    limits.max_archive_bytes = image.len() as u64;
    create::verify_round_trip(&image, &limits, &MetadataLimits::DEFAULT)
        .expect("the package reads back under the same limits");
}
