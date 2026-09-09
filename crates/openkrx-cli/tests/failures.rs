//! Every exit-status category a reader command can reach, end to end.
mod support;

use support::{
    Scratch, consistent_package, malformed_image, missing_attachment_package, one_object, run,
    run_stdin, status, stderr, zip64_image,
};

/// One byte past `Limits::DEFAULT.max_archive_bytes`, the input cap.
const OVER_CAP_BYTES: usize = 64 * 1024 * 1024 + 1;

/// Run one command against an image written to a temporary file.
fn on(command: &str, image: &[u8], json: bool) -> std::process::Output {
    let scratch = Scratch::new(command);
    let path = scratch.file("package.krx", image);
    let path = path.to_str().expect("a UTF-8 temporary path");
    if json {
        run(&[command, path, "--json"])
    } else {
        run(&[command, path])
    }
}

/// The diagnostic a failed JSON response carries.
fn diagnostic(output: &std::process::Output) -> serde_json::Value {
    let value = one_object(output);
    assert_eq!(value["ok"], false);
    assert_eq!(value["verified"], false);
    assert_eq!(value["schema_version"], 1);
    assert!(value["data"].is_null(), "a failure carries no data");
    value["error"].clone()
}

#[test]
fn a_failing_check_is_status_three_for_validate_structure_only() {
    let package = missing_attachment_package();
    let output = on("validate-structure", &package, true);
    assert_eq!(status(&output), 3);
    let value = one_object(&output);
    // `ok` is about the command, not about the package: a report was produced,
    // and the failure is inside it. A consumer reads `summary`, or the exit
    // status, never `ok`. This is documented in the help of both the tool and
    // this command, because the field name invites the other reading.
    assert_eq!(value["ok"], true, "a report was produced");
    assert_eq!(value["data"]["summary"], "inconsistent");
    let failed: Vec<&serde_json::Value> = value["data"]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .filter(|check| check["outcome"] == "fail")
        .collect();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0]["check"], "attachment_references");
    assert_eq!(failed[0]["code"], "metadata.reference.missing_entry");

    for command in ["inspect", "list"] {
        let output = on(command, &package, true);
        assert_eq!(status(&output), 0, "{command} still reports its findings");
    }
}

#[test]
fn a_malformed_image_is_status_six() {
    let output = on("list", &malformed_image(), true);
    assert_eq!(status(&output), 6);
    let error = diagnostic(&output);
    assert_eq!(error["code"], "archive.malformed.eocd_missing");
    assert_eq!(error["category"], "package");
    let line = stderr(&output);
    assert!(line.starts_with("openkrx: archive.malformed.eocd_missing —"));
    assert!(
        line.contains("truncated in transit"),
        "and what to do about it"
    );
    assert!(line.trim_end().ends_with("(exit 6)"));
}

#[test]
fn a_truncated_image_is_status_six_as_well() {
    let package = consistent_package();
    let output = on("inspect", &package[..package.len() - 8], true);
    assert_eq!(status(&output), 6);
    let error = diagnostic(&output);
    assert_eq!(error["category"], "package");
}

#[test]
fn an_unsupported_feature_is_status_seven() {
    let output = on("validate-structure", &zip64_image(), true);
    assert_eq!(status(&output), 7);
    let error = diagnostic(&output);
    assert_eq!(error["code"], "archive.unsupported.zip64");
    assert_eq!(error["category"], "unsupported");
    assert_eq!(error["entry_index"], 0);
}

#[test]
fn an_over_limit_archive_is_status_eight_with_its_numbers() {
    let output = on("list", &support::over_entry_limit_image(), true);
    assert_eq!(status(&output), 8);
    let error = diagnostic(&output);
    assert_eq!(error["code"], "archive.over_limit.entries");
    assert_eq!(error["category"], "limit");
    assert_eq!(error["limit"], 256);
    assert_eq!(error["observed"], 300);
}

#[test]
fn an_unreadable_input_is_status_five() {
    let scratch = Scratch::new("unreadable");
    let missing = scratch.path().join("no-such-package.krx");
    let output = run(&["inspect", missing.to_str().expect("a path"), "--json"]);
    assert_eq!(status(&output), 5);
    let error = diagnostic(&output);
    assert_eq!(error["code"], "input.unreadable");
    assert_eq!(error["category"], "input");
}

#[test]
fn a_directory_named_as_the_input_is_an_input_error_not_a_panic() {
    let scratch = Scratch::new("directory");
    let output = run(&["list", scratch.path().to_str().expect("a path"), "--json"]);
    assert_eq!(status(&output), 5);
    assert_eq!(diagnostic(&output)["code"], "input.unreadable");
}

#[test]
fn an_input_past_the_cap_is_refused_before_parsing() {
    let scratch = Scratch::new("over-cap");
    let path = scratch.file("huge.krx", &vec![0_u8; OVER_CAP_BYTES]);
    let output = run(&["list", path.to_str().expect("a path"), "--json"]);
    assert_eq!(status(&output), 5);
    let error = diagnostic(&output);
    assert_eq!(error["code"], "input.over_limit.archive_bytes");
    assert_eq!(error["category"], "input");
    assert_eq!(error["limit"], 64 * 1024 * 1024_u64);
}

#[test]
fn standard_input_past_the_cap_is_refused_too() {
    let output = run_stdin(&["list", "-", "--json"], &vec![0_u8; OVER_CAP_BYTES]);
    assert_eq!(status(&output), 5);
    assert_eq!(
        diagnostic(&output)["code"],
        "input.over_limit.archive_bytes"
    );
}

#[test]
fn a_failure_in_human_mode_writes_one_line_on_standard_error_and_nothing_else() {
    let output = on("inspect", &malformed_image(), false);
    assert_eq!(status(&output), 6);
    assert!(output.stdout.is_empty(), "no report on stdout");
    let text = stderr(&output);
    assert_eq!(text.lines().count(), 1, "one line, whatever it explains");
    assert!(text.starts_with("openkrx: archive.malformed.eocd_missing"));
    assert!(
        text.contains("(exit 6)"),
        "the status is stated, not implied"
    );
}

#[test]
fn a_failure_in_json_mode_is_still_exactly_one_object_on_standard_output() {
    for command in ["inspect", "list", "validate-structure"] {
        let output = on(command, &malformed_image(), true);
        assert_eq!(status(&output), 6);
        let value = one_object(&output);
        assert_eq!(value["command"], command);
        assert_eq!(value["ok"], false);
        assert_eq!(stderr(&output).lines().count(), 1);
    }
}
