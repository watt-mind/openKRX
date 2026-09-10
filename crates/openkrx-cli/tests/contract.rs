//! The executable's argument surface and its capability report.
mod support;

use support::{one_object, run, status, stderr, stdout};

#[test]
fn capabilities_names_exactly_the_implemented_operations() {
    let output = run(&["capabilities", "--json"]);
    assert_eq!(status(&output), 0);
    assert!(output.stderr.is_empty());
    assert_eq!(
        one_object(&output),
        serde_json::json!({
            "schema_version": 1,
            "ok": true,
            "command": "capabilities",
            "data": {
                "project": "openKRX",
                "stage": "reader-writer",
                "operations": [
                    "inspect",
                    "list",
                    "validate-structure",
                    "extract",
                    "create",
                    "repack",
                ],
            },
            "verified": false,
        })
    );
}

#[test]
fn human_capabilities_state_the_boundary_rather_than_a_verdict() {
    let output = run(&["capabilities"]);
    assert_eq!(status(&output), 0);
    assert!(output.stderr.is_empty());
    let text = stdout(&output);
    assert!(text.contains("openKRX: reader-writer"));
    assert!(text.contains("inspect, list, validate-structure, extract, create"));
    assert!(text.contains("not signature verification"));
    assert!(text.contains("Nothing is verified"));
}

#[test]
fn help_lists_every_command_and_the_exit_statuses() {
    let output = run(&["--help"]);
    assert_eq!(status(&output), 0);
    let text = stdout(&output);
    for command in [
        "capabilities",
        "inspect",
        "list",
        "validate-structure",
        "extract",
        "create",
    ] {
        assert!(text.contains(command), "help must mention {command}");
    }
    assert!(text.contains("Exit statuses"));
}

#[test]
fn each_command_documents_its_file_argument_and_json_flag() {
    for command in ["inspect", "list", "validate-structure", "extract"] {
        let output = run(&[command, "--help"]);
        assert_eq!(status(&output), 0);
        let text = stdout(&output);
        assert!(text.contains("FILE"), "{command} help names its argument");
        assert!(text.contains("--json"), "{command} help offers --json");
        assert!(
            text.contains("standard input"),
            "{command} help explains `-`"
        );
    }
}

#[test]
fn the_help_says_which_statuses_belong_to_which_command() {
    // A blanket "3 means a check failed" would be read as true of `inspect`
    // too, and a reader checking `$?` after `inspect` would take a broken
    // package for a clean one.
    let top = stdout(&run(&["--help"]));
    assert!(top.contains("validate-structure only: a structural check failed"));
    assert!(top.contains("validate-structure only: nothing failed"));
    assert!(top.contains("inspect and list exit 0 whenever they produce"));
    assert!(
        top.contains("extract, create and repack only: the destination could not be used"),
        "status 9 belongs to the three commands that write"
    );
    assert!(
        top.contains("`ok` says only that the command produced a report"),
        "the envelope's `ok` field is explained where it can be misread"
    );

    let inspect = stdout(&run(&["inspect", "--help"]));
    assert!(inspect.contains("exits 0 whenever it produces its report"));
    assert!(inspect.contains("the check table it prints"));
    assert!(
        inspect.contains("Use\nvalidate-structure") || inspect.contains("Use validate-structure")
    );

    // `list` runs no structural check, so its note must not claim a failed
    // check is somewhere in its report: the report is the entry listing.
    let list = stdout(&run(&["list", "--help"]));
    assert!(list.contains("runs no structural check"));
    assert!(list.contains("Use inspect to see the checks"));
    assert!(
        !list.contains("the outcome is in the report"),
        "list must not describe a check outcome it never prints"
    );

    let validate = stdout(&run(&["validate-structure", "--help"]));
    assert!(validate.contains("3 when a check failed"));
    assert!(validate.contains("stays true when a check failed"));
}

#[test]
fn version_is_the_package_version() {
    let output = run(&["--version"]);
    assert_eq!(status(&output), 0);
    assert_eq!(
        stdout(&output).trim(),
        concat!("openkrx ", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn argument_errors_exit_with_the_usage_status() {
    let cases: [&[&str]; 9] = [
        &[],
        &["extract"],
        &["capabilities", "--unknown"],
        &["inspect"],
        &["list", "--json"],
        &["validate-structure", "a", "b"],
        // `create` needs a manifest, and exactly one destination.
        &["create"],
        &["create", "--manifest", "m.json"],
        &[
            "create",
            "--manifest",
            "m.json",
            "--out",
            "p.krx",
            "--stdout",
        ],
    ];
    for args in cases {
        let output = run(args);
        assert_eq!(status(&output), 2, "usage status for {args:?}");
        assert!(output.stdout.is_empty(), "no report for {args:?}");
        assert!(!stderr(&output).is_empty(), "a message for {args:?}");
    }
}
