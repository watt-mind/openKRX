//! Diagnostics never carry the input path, an entry name or a declared value.
//!
//! The rule is stated in `SECURITY.md` and in
//! `docs/architecture.md#command-contract-and-json-envelope`: a diagnostic
//! carries a stable code, a category, an entry index and numbers. This file
//! holds it by putting a distinctive canary in each of the three places and
//! searching the output for it — standard error in every case, and standard
//! output whenever the run failed.
mod support;

use support::{
    CANARY_ENTRY, CANARY_PATH, CANARY_VALUE, Scratch, canary_package, malformed_image, run, status,
    stderr, stdout,
};

/// Every canary that must never appear in a diagnostic.
const CANARIES: [&str; 3] = [CANARY_PATH, CANARY_ENTRY, CANARY_VALUE];

/// Assert that no canary appears anywhere in `text`.
fn clean(text: &str, what: &str) {
    for canary in CANARIES {
        assert!(
            !text.contains(canary),
            "{what} disclosed {canary}: {text:?}"
        );
    }
}

#[test]
fn standard_error_never_carries_a_canary_whatever_happened() {
    let scratch = Scratch::new("privacy");
    for (label, image) in [
        ("readable", canary_package()),
        ("malformed", malformed_image()),
    ] {
        let path = scratch.canary_file(&format!("{label}-{CANARY_ENTRY}"), &image);
        let path = path.to_str().expect("a UTF-8 temporary path");
        for command in ["inspect", "list", "validate-structure"] {
            for json in [true, false] {
                let mut args = vec![command, path];
                if json {
                    args.push("--json");
                }
                let output = run(&args);
                clean(
                    &stderr(&output),
                    &format!("{command} on a {label} package (json={json}) stderr"),
                );
            }
        }
    }
}

#[test]
fn a_failed_run_never_carries_a_canary_on_standard_output_either() {
    let scratch = Scratch::new("privacy-failure");
    let path = scratch.canary_file(CANARY_ENTRY, &malformed_image());
    let path = path.to_str().expect("a UTF-8 temporary path");
    for command in ["inspect", "list", "validate-structure"] {
        for json in [true, false] {
            let mut args = vec![command, path];
            if json {
                args.push("--json");
            }
            let output = run(&args);
            assert_ne!(status(&output), 0, "{command} failed as intended");
            clean(
                &stdout(&output),
                &format!("{command} (json={json}) stdout on failure"),
            );
        }
    }
}

#[test]
fn the_input_path_is_absent_from_a_successful_report_as_well() {
    let scratch = Scratch::new("privacy-path");
    let path = scratch.canary_file("package.krx", &canary_package());
    let path = path.to_str().expect("a UTF-8 temporary path");
    for command in ["inspect", "list", "validate-structure"] {
        let output = run(&[command, path, "--json"]);
        assert!(
            !stdout(&output).contains(CANARY_PATH),
            "{command} must not echo the input path"
        );
    }
}

#[test]
fn a_declared_value_reaches_standard_output_only_for_inspect() {
    let scratch = Scratch::new("privacy-value");
    let path = scratch.file("package.krx", &canary_package());
    let path = path.to_str().expect("a UTF-8 temporary path");

    let inspected = stdout(&run(&["inspect", path]));
    assert!(
        inspected.contains(CANARY_VALUE),
        "inspect was asked for the declared values and prints them"
    );

    for command in ["list", "validate-structure"] {
        let text = stdout(&run(&[command, path]));
        assert!(
            !text.contains(CANARY_VALUE),
            "{command} must not print a declared metadata value"
        );
    }
}
