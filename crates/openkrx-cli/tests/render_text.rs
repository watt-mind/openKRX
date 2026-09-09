//! The human renderer's own contract, line by line.
//!
//! `reader.rs` holds what each command reports; this file holds how the text
//! mode *says* it. The distinction matters because the JSON mode is asserted
//! field by field while the text mode was asserted only by substring, so a
//! renderer could drop an attachment block, lose an entry index or invent a
//! cut marker without any test noticing. Mutation testing found exactly that
//! (see docs/testing.md, "Mutation testing"), and every assertion here pins a
//! line whose absence or wrong number a mutant would otherwise survive.
//!
//! The assertions are on whole lines rather than fragments, because the
//! alignment is part of what makes the block readable, and a fragment match
//! cannot tell a rendered value from a rendered placeholder.
mod support;

use openkrx_core::synthetic::meta::{Attachment, Document, METADATA_FILE, krx};
use openkrx_core::synthetic::{Archive, Entry};
use support::{Scratch, attachment_package, consistent_package, one_object, run, status, stdout};

/// Run one reader command against a package written to a temporary file.
fn on(command: &str, package: &[u8], json: bool) -> std::process::Output {
    let scratch = Scratch::new(command);
    let path = scratch.file("package.krx", package);
    let path = path.to_str().expect("a UTF-8 temporary path");
    if json {
        run(&[command, path, "--json"])
    } else {
        run(&[command, path])
    }
}

/// The text-mode standard output of a successful run.
fn text(command: &str, package: &[u8]) -> String {
    let output = on(command, package, false);
    assert!(
        matches!(status(&output), 0 | 3 | 4),
        "{command} produced a report"
    );
    stdout(&output)
}

/// Assert that `haystack` holds `line` as a complete line of its own.
fn has_line(haystack: &str, line: &str) {
    assert!(
        haystack.lines().any(|candidate| candidate == line),
        "expected the line {line:?} in:\n{haystack}"
    );
}

#[test]
fn inspect_text_renders_each_attachment_and_the_entry_it_resolves_to() {
    let text = text("inspect", &attachment_package());
    has_line(
        &text,
        "  attachment 1          KRX/OCD/Payload/ID-1/synthetic.pdf",
    );
    has_line(
        &text,
        "    declared size       12.5 (unit unresolved, rule M13)",
    );
    // The observed decoded size is the fact that separates a resolved
    // reference from a declared one, so the phrase carries the number.
    has_line(&text, "    resolves to         entry 2, 19 bytes decoded");
    assert!(
        !text.contains("no entry of this archive has that name"),
        "a resolved attachment is never reported as missing"
    );
}

#[test]
fn inspect_text_says_when_a_declared_attachment_has_no_entry() {
    let text = text("inspect", &support::missing_attachment_package());
    has_line(
        &text,
        "  attachment 1          KRX/OCD/Payload/ID-1/synthetic.pdf",
    );
    has_line(
        &text,
        "    resolves to         no entry of this archive has that name",
    );
    assert!(
        !text.contains("bytes decoded"),
        "nothing is observed for a reference that resolves to nothing"
    );
}

#[test]
fn inspect_text_names_the_metadata_entry_with_its_index_and_the_marker_outcome() {
    let text = text("inspect", &attachment_package());
    has_line(
        &text,
        "  metadata entry        KRX/OCD/Metalayer/KULDEMENY_META.xml (entry 1)",
    );
    has_line(&text, "  format marker         pass");
    has_line(&text, "  entries               3");
}

#[test]
fn validate_structure_text_counts_the_checks_that_failed_and_the_ones_left_open() {
    // Eleven checks, none failing, one undecided under rule M13.
    let undecided = text("validate-structure", &attachment_package());
    has_line(
        &undecided,
        "0 of 11 checks failed and 1 could not be decided. This is not a statement \
that the package is a valid or conforming KRX file.",
    );

    // The same package shape with no declared attachment leaves nothing open.
    let settled = text("validate-structure", &consistent_package());
    has_line(
        &settled,
        "0 of 11 checks failed and 0 could not be decided. This is not a statement \
that the package is a valid or conforming KRX file.",
    );
}

#[test]
fn a_check_that_does_not_apply_is_reported_as_such_rather_than_as_undecided() {
    // A package declaring no attachment cannot have its attachment checks
    // decided either way: they do not apply. Folding that into "unresolved"
    // would make a complete package look like one with an open rule, and
    // would change the summary a caller branches on.
    let value = one_object(&on("validate-structure", &consistent_package(), true));
    assert_eq!(value["data"]["summary"], "consistent");
    let checks = value["data"]["checks"]
        .as_array()
        .expect("the check list")
        .clone();
    let names: Vec<&str> = checks
        .iter()
        .filter(|check| check["outcome"] == "not_applicable")
        .map(|check| check["check"].as_str().expect("a check name"))
        .collect();
    assert_eq!(
        names,
        [
            "handling_instructions_form",
            "attachment_references",
            "attachment_count",
            "attachment_uniqueness",
            "declared_size",
        ],
        "each check that cannot apply says so"
    );
    assert!(
        value["data"]["unresolved_rules"]
            .as_array()
            .expect("the rule list")
            .is_empty(),
        "a check that does not apply cites no undecided rule"
    );

    let text = text("validate-structure", &consistent_package());
    has_line(&text, "  handling_instructions_form  not applicable");
    has_line(&text, "No rule was left undecided.");
}

#[test]
fn a_name_at_the_cut_limit_keeps_its_last_character_and_gains_no_marker() {
    // `render::human::MAX_UNITS` is 200. The prefix below is 21 characters,
    // so 179 more make a name of exactly the limit: nothing is dropped, and
    // the remainder marker must not appear. One character more drops one.
    let at_limit = long_name_package(179);
    let over_limit = long_name_package(180);

    let at = text("list", &at_limit);
    assert!(
        !at.contains("…[+"),
        "a name exactly at the limit is not marked as cut:\n{at}"
    );
    assert!(
        at.contains(&format!("KRX/OCD/Payload/ID-1/{}", "n".repeat(179))),
        "the whole name is printed"
    );

    let over = text("list", &over_limit);
    assert!(
        over.contains("…[+1]"),
        "one character over drops one:\n{over}"
    );
}

#[test]
fn the_declared_attachment_count_is_the_number_the_document_states() {
    // Two declared attachments, so a renderer that hard-codes one, or that
    // reports the number of `MELLEKLET` elements instead of the declared
    // `MELLEKLETEK_SZAMA`, disagrees with the document.
    let document = Document {
        declared_count: Some("2".to_owned()),
        attachments: vec![
            Attachment::new(1, "synthetic.pdf", "KRX/OCD/Payload/ID-1"),
            Attachment::new(2, "second.pdf", "KRX/OCD/Payload/ID-2"),
        ],
        ..Document::default()
    };
    let package = krx(
        "KRX/OCD/",
        METADATA_FILE,
        &document.bytes(),
        &[
            "KRX/OCD/Payload/ID-1/synthetic.pdf",
            "KRX/OCD/Payload/ID-2/second.pdf",
        ],
    );

    let value = one_object(&on("inspect", &package, true));
    assert_eq!(value["data"]["metadata"]["declared_attachment_count"], 2);
    assert_eq!(
        value["data"]["metadata"]["attachments"]
            .as_array()
            .expect("the attachment list")
            .len(),
        2
    );

    let text = text("inspect", &package);
    has_line(&text, "  attachments declared  2");
}

/// A readable package whose third entry's name is `filler` characters long
/// after the payload prefix.
fn long_name_package(filler: usize) -> Vec<u8> {
    let mut name = b"KRX/OCD/Payload/ID-1/".to_vec();
    name.extend(std::iter::repeat_n(b'n', filler));
    Archive::of(vec![
        Entry::stored(b"mimetype", openkrx_core::synthetic::meta::MARKER_CONTENT),
        Entry::deflated(
            format!("KRX/OCD/Metalayer/{METADATA_FILE}").as_bytes(),
            &Document::header_only().bytes(),
        ),
        Entry::stored(&name, b"synthetic payload"),
    ])
    .build()
}

#[test]
fn an_attachment_matched_only_after_a_prefix_swap_says_so_and_cites_the_rule() {
    // The declared path uses the observed root prefix `KRX/OCD/`, but the
    // entry is stored under `OCD/`. Rule M14 leaves that reading open, and
    // both output modes must distinguish it from a reference that resolved
    // exactly and from one that resolved to nothing at all.
    let document = Document {
        attachments: vec![Attachment::new(1, "synthetic.pdf", "KRX/OCD/Payload/ID-1")],
        ..Document::default()
    };
    let package = Archive::of(vec![
        Entry::stored(b"mimetype", openkrx_core::synthetic::meta::MARKER_CONTENT),
        Entry::deflated(
            format!("KRX/OCD/Metalayer/{METADATA_FILE}").as_bytes(),
            &document.bytes(),
        ),
        Entry::stored(b"OCD/Payload/ID-1/synthetic.pdf", b"synthetic payload 0"),
    ])
    .build();

    let value = one_object(&on("inspect", &package, true));
    let attachment = &value["data"]["metadata"]["attachments"][0];
    assert_eq!(attachment["resolution"], "prefix_variant");
    assert_eq!(attachment["entry_index"], 2);

    let text = text("inspect", &package);
    has_line(
        &text,
        "    resolves to         entry 2, 19 bytes decoded, matched only after \
adjusting the root prefix (rule M14)",
    );
}
