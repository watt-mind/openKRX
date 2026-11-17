//! The human renderer's agreement in number, at the one boundary it has.
//!
//! Every count a summary prints is followed by a noun, and the noun is
//! singular for exactly one. A package is never one byte long and an edit
//! seldom sets one header field, so the subprocess tests in
//! `tests/render_text.rs` never reach the singular for those counts; these
//! render the reports from hand-built data and pin the whole line on both
//! sides of the boundary.

use super::human;
use crate::commands::create::{CreateData, LAYOUT};
use crate::commands::repack::RepackData;

fn create_data(bytes_written: u64) -> CreateData {
    CreateData {
        bytes_written,
        entries: 3,
        unresolved_rules: Vec::new(),
        layout: LAYOUT,
    }
}

fn repack_data(header_fields: Vec<&'static str>, bytes_written: u64) -> RepackData {
    RepackData {
        bytes_written,
        entries: 4,
        preserved: vec![1],
        changed: Vec::new(),
        added: vec![2, 3],
        removed: Vec::new(),
        header_fields,
        unresolved_rules: Vec::new(),
        layout: LAYOUT,
    }
}

fn first_line(report: &str) -> &str {
    report.lines().next().expect("a report has a first line")
}

#[test]
fn a_written_size_of_one_is_one_byte_and_any_other_is_bytes() {
    assert_eq!(
        first_line(&human::create(&create_data(1), false)),
        "Wrote 1 byte in 3 entries."
    );
    assert_eq!(
        first_line(&human::create(&create_data(2), false)),
        "Wrote 2 bytes in 3 entries."
    );
    assert_eq!(
        first_line(&human::repack(&repack_data(Vec::new(), 1), true)),
        "Wrote 1 byte in 4 entries."
    );
    assert_eq!(
        first_line(&human::repack(&repack_data(Vec::new(), 0), true)),
        "Wrote 0 bytes in 4 entries."
    );
}

#[test]
fn one_header_field_is_a_field_and_two_are_fields() {
    let one = human::repack(&repack_data(vec!["targy"], 512), false);
    assert!(
        one.lines()
            .any(|line| line == "Header field set: targy. No value is repeated here."),
        "{one}"
    );
    let two = human::repack(&repack_data(vec!["targy", "hivatkozas"], 512), false);
    assert!(
        two.lines()
            .any(|line| line == "Header fields set: targy, hivatkozas. No value is repeated here."),
        "{two}"
    );
}

#[test]
fn one_attachment_number_is_an_attachment_and_two_are_attachments() {
    let report = human::repack(&repack_data(Vec::new(), 512), false);
    assert!(
        report.lines().any(|line| line
            == "In the package that was edited: 1 attachment preserved byte for byte (1), \
none replaced, none removed."),
        "{report}"
    );
    assert!(
        report
            .lines()
            .any(|line| line == "In the package that was written: 2 attachments added (2, 3)."),
        "{report}"
    );
}
