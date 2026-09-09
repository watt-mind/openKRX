//! The three reader commands on packages that can be read.
mod support;

use support::{
    Scratch, attachment_package, consistent_package, control_name_package, layout_package,
    non_utf8_name_package, one_object, run, run_stdin, status, stderr, stdout,
};

/// Run one command against a package written to a temporary file.
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

#[test]
fn a_consistent_package_reports_every_command_successfully_in_json() {
    for command in ["inspect", "list", "validate-structure"] {
        let output = on(command, &consistent_package(), true);
        assert_eq!(status(&output), 0, "{command} succeeds");
        assert!(output.stderr.is_empty(), "{command} leaves stderr empty");
        let value = one_object(&output);
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["ok"], true);
        assert_eq!(value["command"], command);
        assert_eq!(value["verified"], false);
        assert!(value["data"].is_object(), "{command} carries data");
        for forbidden in ["valid", "conforming", "is_krx"] {
            assert!(
                !stdout(&output).contains(&format!("\"{forbidden}\"")),
                "{command} must not report a {forbidden} field"
            );
        }
    }
}

#[test]
fn a_consistent_package_reports_every_command_successfully_in_text() {
    for command in ["inspect", "list", "validate-structure"] {
        let output = on(command, &consistent_package(), false);
        assert_eq!(status(&output), 0, "{command} succeeds");
        assert!(output.stderr.is_empty(), "{command} leaves stderr empty");
        let text = stdout(&output);
        assert!(text.contains("Nothing is verified"), "{command} boundary");
        assert!(!text.is_empty());
    }
}

#[test]
fn list_reports_every_entry_in_central_directory_order() {
    let output = on("list", &attachment_package(), true);
    assert_eq!(status(&output), 0);
    let value = one_object(&output);
    let entries = value["data"]["entries"]
        .as_array()
        .expect("an entry array")
        .clone();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0]["index"], 0);
    assert_eq!(entries[0]["name"], "mimetype");
    assert_eq!(entries[0]["method"], "stored");
    assert_eq!(entries[0]["decoded_size"], 19);
    assert_eq!(entries[0]["declared_size"], 19);
    assert_eq!(entries[1]["method"], "deflate");
    assert_eq!(entries[1]["name"], "KRX/OCD/Metalayer/KULDEMENY_META.xml");
    assert_eq!(entries[2]["name"], "KRX/OCD/Payload/ID-1/synthetic.pdf");
    for entry in &entries {
        let crc = entry["crc32"].as_str().expect("a crc32 string");
        assert_eq!(crc.len(), 8, "crc32 is eight hex digits");
        assert!(crc.chars().all(|digit| digit.is_ascii_hexdigit()));
        assert!(entry["name_hex"].is_null(), "a UTF-8 name carries no hex");
        assert!(entry["name_utf8_flag"].is_boolean());
    }
}

#[test]
fn list_ordering_is_the_same_on_every_run() {
    let package = attachment_package();
    let first = stdout(&on("list", &package, true));
    let second = stdout(&on("list", &package, true));
    assert_eq!(first, second);
}

#[test]
fn inspect_reports_the_declared_document_beside_the_observed_archive() {
    let output = on("inspect", &attachment_package(), true);
    assert_eq!(status(&output), 0);
    let value = one_object(&output);
    let data = &value["data"];
    assert_eq!(data["observations"]["root_prefix"], "KRX/OCD/");
    assert_eq!(
        data["observations"]["metadata_entry_name"],
        "KRX/OCD/Metalayer/KULDEMENY_META.xml"
    );
    assert_eq!(data["observations"]["metadata_entry_index"], 1);
    assert_eq!(data["observations"]["marker"]["outcome"], "pass");
    assert_eq!(data["observations"]["entry_count"], 3);

    let metadata = &data["metadata"];
    assert_eq!(metadata["version"], "v0.9");
    assert_eq!(metadata["source_system"], "KER");
    assert_eq!(metadata["consignment_type"], "KULDEMENY");
    assert_eq!(metadata["consignment_id"], "SYN-0001");
    assert_eq!(metadata["created_at"], "2026-01-02T03:04:05.000+01:00");
    assert_eq!(metadata["declared_attachment_count"], 1);

    let attachment = &metadata["attachments"][0];
    assert_eq!(attachment["number"], 1);
    assert_eq!(
        attachment["declared_path"],
        "KRX/OCD/Payload/ID-1/synthetic.pdf"
    );
    assert_eq!(attachment["resolution"], "resolved");
    assert_eq!(attachment["entry_index"], 2);
    assert_eq!(attachment["declared_size_text"], "12.5");
    assert!(attachment["observed_size"].is_u64());

    let checks = data["checks"].as_array().expect("a check array");
    assert_eq!(checks.len(), 11);
    assert_eq!(checks[0]["check"], "metadata_location");
    assert_eq!(checks[10]["check"], "declared_size");
    assert_eq!(checks[10]["outcome"], "unresolved");
    assert_eq!(checks[10]["rule"], "M13");
}

#[test]
fn inspect_text_states_that_a_declared_value_is_only_declared() {
    let output = on("inspect", &attachment_package(), false);
    assert_eq!(status(&output), 0);
    let text = stdout(&output);
    assert!(text.contains("Declared metadata"));
    assert!(text.contains("SYN-0001"));
    assert!(text.contains("undecided (rule M13)"));
    assert!(!text.to_lowercase().contains("is valid"));
}

#[test]
fn validate_structure_renders_each_outcome_rather_than_a_verdict() {
    let output = on("validate-structure", &attachment_package(), true);
    assert_eq!(status(&output), 4, "an undecided rule is status 4");
    let value = one_object(&output);
    assert_eq!(value["data"]["summary"], "unresolved");
    assert_eq!(value["data"]["unresolved_rules"][0], "M13");
    let checks = value["data"]["checks"].as_array().expect("checks");
    assert_eq!(checks.len(), 11);
    let outcomes: Vec<&str> = checks
        .iter()
        .map(|check| check["outcome"].as_str().expect("an outcome"))
        .collect();
    assert!(outcomes.contains(&"pass"));
    assert!(outcomes.contains(&"unresolved"));
    assert!(!outcomes.contains(&"fail"));
}

#[test]
fn validate_structure_text_says_what_the_summary_does_not_claim() {
    let output = on("validate-structure", &attachment_package(), false);
    assert_eq!(status(&output), 4);
    let text = stdout(&output);
    assert!(text.contains("Structural summary: unresolved"));
    assert!(text.contains("not a statement that the package is a valid"));
    assert!(text.contains("Undecided rules: M13"));
}

#[test]
fn the_other_two_layouts_leave_the_root_prefix_rule_undecided() {
    for prefix in ["OCD/", ""] {
        let output = on("validate-structure", &layout_package(prefix), true);
        assert_eq!(status(&output), 4, "layout {prefix:?} is undecided");
        let value = one_object(&output);
        assert_eq!(value["data"]["summary"], "unresolved");
        let rules = value["data"]["unresolved_rules"]
            .as_array()
            .expect("unresolved rules");
        assert!(
            rules.iter().any(|rule| rule == "A19"),
            "layout {prefix:?} cites A19"
        );
    }
}

#[test]
fn an_empty_root_prefix_is_stated_rather_than_left_blank() {
    // One of the three layouts rule A19 describes puts `Metalayer/` at the
    // archive root, so an empty prefix is an observation, not a missing value.
    let output = on("inspect", &layout_package(""), false);
    assert_eq!(status(&output), 0);
    assert!(
        stdout(&output)
            .contains("root prefix           none; the metadata entry is at the archive root"),
        "an empty prefix is named"
    );
    let value = one_object(&on("inspect", &layout_package(""), true));
    assert_eq!(value["data"]["observations"]["root_prefix"], "");
}

#[test]
fn standard_input_is_read_by_every_command() {
    let package = consistent_package();
    for command in ["inspect", "list", "validate-structure"] {
        let output = run_stdin(&[command, "-", "--json"], &package);
        assert_eq!(status(&output), 0, "{command} reads standard input");
        assert!(output.stderr.is_empty());
        assert_eq!(one_object(&output)["command"], command);
    }
}

#[test]
fn standard_input_and_a_file_produce_the_same_report() {
    let package = attachment_package();
    let from_file = stdout(&on("inspect", &package, true));
    let from_stdin = stdout(&run_stdin(&["inspect", "-", "--json"], &package));
    assert_eq!(from_file, from_stdin);
}

#[test]
fn a_name_that_is_not_utf8_is_reported_as_bytes_rather_than_guessed() {
    let package = non_utf8_name_package();
    let value = one_object(&on("list", &package, true));
    let entry = &value["data"]["entries"][2];
    assert!(entry["name"].is_null(), "no invented text form");
    assert_eq!(
        entry["name_hex"],
        "4b52582f4f43442f5061796c6f61642f49442d312ffffe2e62696e"
    );

    let text = stdout(&on("list", &package, false));
    assert!(text.contains("\\x{ff}\\x{fe}"), "raw bytes are escaped");
    assert!(text.contains("[name is not UTF-8]"));
}

#[test]
fn an_invisible_character_in_a_name_never_reaches_the_terminal() {
    let output = on("list", &control_name_package(), false);
    assert_eq!(status(&output), 0);
    let text = stdout(&output);
    assert!(text.contains("\\u{0085}"), "a C1 control is escaped");
    assert!(text.contains("\\u{202e}"), "a bidi override is escaped");
    assert!(!text.contains('\u{0085}'));
    assert!(!text.contains('\u{202e}'));
}

#[test]
fn a_long_name_is_cut_and_the_remainder_is_counted() {
    let mut name = b"KRX/OCD/Payload/ID-1/".to_vec();
    name.extend(std::iter::repeat_n(b'n', 220));
    let package = {
        use openkrx_core::synthetic::meta::{Document, MARKER_CONTENT, METADATA_FILE};
        use openkrx_core::synthetic::{Archive, Entry};
        Archive::of(vec![
            Entry::stored(b"mimetype", MARKER_CONTENT),
            Entry::deflated(
                format!("KRX/OCD/Metalayer/{METADATA_FILE}").as_bytes(),
                &Document::header_only().bytes(),
            ),
            Entry::stored(&name, b"synthetic payload"),
        ])
        .build()
    };
    let text = stdout(&on("list", &package, false));
    assert!(text.contains("…[+41]"), "the dropped count is stated");
    assert!(!text.contains(&"n".repeat(200)));

    // The machine-readable form is never truncated: a consumer needs the name.
    let value = one_object(&on("list", &package, true));
    let name = value["data"]["entries"][2]["name"]
        .as_str()
        .expect("a name")
        .to_owned();
    assert_eq!(name.len(), 241);
}

#[test]
fn a_successful_run_writes_nothing_at_all_on_standard_error() {
    for command in ["inspect", "list", "validate-structure"] {
        for json in [true, false] {
            let output = on(command, &attachment_package(), json);
            assert_eq!(
                stderr(&output),
                "",
                "{command} json={json} keeps stderr empty"
            );
        }
    }
}
