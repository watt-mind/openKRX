//! `create`: the manifest, the package it produces, and every refusal.
//!
//! Every test here runs the executable as a subprocess over a manifest and
//! attachment files it wrote itself, because that is the whole contract: the
//! JSON a person typed goes in, and either a package and a report come out, or
//! nothing is written at all.
//!
//! Three properties carry the most weight, and each has its own test rather
//! than being asserted in passing:
//!
//! - **Determinism.** Two runs over the same manifest and the same files
//!   produce byte-identical packages, on any machine, at any time. openKRX has
//!   no clock, so the only time in the output is the manifest's own.
//! - **The round trip.** A package `create` wrote is one openKRX reads: `list`
//!   sees the entries the layout documents, and `validate-structure` exits `4`
//!   citing the unresolved rules — never `3`. That is the definition of
//!   success, and a `3` here would be a defect in the writer.
//! - **No clobbering, and no partial file.** `--out` must not exist in any
//!   form, and a run that fails after creating it removes it again.
mod support;

use std::path::Path;

use support::{
    CANARY_ENTRY, CANARY_PATH, CANARY_VALUE, MANIFEST_TIMESTAMP, Scratch, manifest, one_object,
    run, status, stderr, stdout,
};

/// Write a manifest and the attachment files a test names, and return its path.
fn scene(scratch: &Scratch, attachments: &str, files: &[(&str, &[u8])]) -> std::path::PathBuf {
    for (name, bytes) in files {
        let _ = scratch.file(name, bytes);
    }
    scratch.file("manifest.json", manifest(attachments).as_bytes())
}

/// Run `create` over `manifest` into `out`, in JSON mode.
fn create_json(manifest: &Path, out: &Path) -> std::process::Output {
    run(&[
        "create",
        "--manifest",
        manifest.to_str().expect("a UTF-8 manifest path"),
        "--out",
        out.to_str().expect("a UTF-8 output path"),
        "--json",
    ])
}

/// The same, in human mode.
fn create(manifest: &Path, out: &Path) -> std::process::Output {
    run(&[
        "create",
        "--manifest",
        manifest.to_str().expect("a UTF-8 manifest path"),
        "--out",
        out.to_str().expect("a UTF-8 output path"),
    ])
}

/// The `error` object of a refusal, which every failure test reads.
fn diagnostic(output: &std::process::Output) -> serde_json::Value {
    one_object(output)
        .get("error")
        .cloned()
        .expect("a failed envelope carries error")
}

/// Run `validate-structure` over a path and return its status and report.
fn validate(path: &Path) -> (i32, serde_json::Value) {
    let output = run(&[
        "validate-structure",
        path.to_str().expect("a UTF-8 package path"),
        "--json",
    ]);
    (status(&output), one_object(&output))
}

#[test]
fn a_manifest_with_no_attachment_writes_the_two_fixed_entries() {
    let scratch = Scratch::new("create-empty");
    let manifest = scene(&scratch, "", &[]);
    let out = scratch.path().join("package.krx");
    let output = create_json(&manifest, &out);
    assert_eq!(status(&output), 0);
    assert!(output.stderr.is_empty(), "a successful run says nothing");
    let response = one_object(&output);
    assert_eq!(response["command"], "create");
    assert_eq!(response["verified"], false);
    let data = &response["data"];
    assert_eq!(data["entries"], 2, "the marker and the metadata document");
    assert_eq!(data["layout"], "canonical-documented");
    assert_eq!(
        data["bytes_written"].as_u64().expect("a byte count"),
        std::fs::metadata(&out).expect("the package exists").len(),
        "the report counts the bytes that are on disk"
    );
    // A19 alone: the marker's place is unresolved, and with no attachment
    // there is no declared size for M13 to be unresolved about.
    assert_eq!(data["unresolved_rules"], serde_json::json!(["A19"]));
}

#[test]
fn a_manifest_with_three_attachments_places_and_numbers_each_one() {
    let scratch = Scratch::new("create-three");
    let manifest = scene(
        &scratch,
        "{\"path\":\"a.txt\",\"description\":\"first\"},\
{\"path\":\"b.txt\",\"description\":\"second\"},\
{\"path\":\"c.bin\",\"file_name\":\"renamed.bin\",\"description\":\"third\"}",
        &[
            ("a.txt", b"alpha"),
            ("b.txt", b"beta"),
            ("c.bin", b"gamma bytes"),
        ],
    );
    let out = scratch.path().join("package.krx");
    assert_eq!(status(&create_json(&manifest, &out)), 0);

    let listing = one_object(&run(&[
        "list",
        out.to_str().expect("a UTF-8 package path"),
        "--json",
    ]));
    let names: Vec<String> = listing["data"]["entries"]
        .as_array()
        .expect("an entry array")
        .iter()
        .map(|entry| entry["name"].as_str().expect("a UTF-8 name").to_owned())
        .collect();
    assert_eq!(
        names,
        vec![
            "KRX/OCD/mimetype",
            "KRX/OCD/Metalayer/KULDEMENY_META.xml",
            "KRX/OCD/Payload/ID-1/a.txt",
            "KRX/OCD/Payload/ID-2/b.txt",
            "KRX/OCD/Payload/ID-3/renamed.bin",
        ],
        "the layout numbers each attachment and honours file_name"
    );
}

#[test]
fn a_created_package_reads_back_as_unresolved_and_never_as_inconsistent() {
    // The definition of success: exit 4, citing rules no source settles, and
    // never exit 3. A failing check over a package openkrx itself wrote would
    // be a defect in the writer.
    let scratch = Scratch::new("create-round-trip");
    let manifest = scene(
        &scratch,
        "{\"path\":\"a.txt\",\"description\":\"the only attachment\"}",
        &[("a.txt", b"alpha")],
    );
    let out = scratch.path().join("package.krx");
    assert_eq!(status(&create_json(&manifest, &out)), 0);

    let (code, report) = validate(&out);
    assert_eq!(code, 4, "unresolved, never inconsistent");
    assert_eq!(report["data"]["summary"], "unresolved");
    assert_eq!(
        report["data"]["unresolved_rules"],
        serde_json::json!(["A19", "M13"]),
        "the marker's place and the declared size's unit"
    );
    for check in report["data"]["checks"].as_array().expect("the checks") {
        assert_ne!(check["outcome"], "fail", "no check may fail: {check}");
    }

    // inspect reads the same package without complaint, and reports what the
    // document declares about the attachment the writer derived.
    let inspected = one_object(&run(&[
        "inspect",
        out.to_str().expect("a UTF-8 package path"),
        "--json",
    ]));
    let attachment = &inspected["data"]["metadata"]["attachments"][0];
    assert_eq!(attachment["declared_path"], "KRX/OCD/Payload/ID-1/a.txt");
    assert_eq!(attachment["resolution"], "resolved");
}

#[test]
fn two_runs_over_the_same_manifest_write_the_same_bytes() {
    let scratch = Scratch::new("create-deterministic");
    let manifest = scene(
        &scratch,
        "{\"path\":\"a.txt\",\"description\":\"first\"}",
        &[("a.txt", b"alpha")],
    );
    let first = scratch.path().join("first.krx");
    let second = scratch.path().join("second.krx");
    assert_eq!(status(&create_json(&manifest, &first)), 0);
    assert_eq!(status(&create_json(&manifest, &second)), 0);
    assert_eq!(
        std::fs::read(&first).expect("the first package"),
        std::fs::read(&second).expect("the second package"),
        "the same manifest and the same files produce the same bytes"
    );
}

#[test]
fn the_extraction_of_a_created_package_returns_the_attachment_bytes() {
    let scratch = Scratch::new("create-extract");
    let manifest = scene(
        &scratch,
        "{\"path\":\"a.txt\",\"description\":\"the only attachment\"}",
        &[("a.txt", b"alpha")],
    );
    let out = scratch.path().join("package.krx");
    assert_eq!(status(&create_json(&manifest, &out)), 0);
    let into = scratch.dir("extracted");
    let extracted = run(&[
        "extract",
        out.to_str().expect("a UTF-8 package path"),
        "--into",
        into.to_str().expect("a UTF-8 destination"),
        "--json",
    ]);
    assert_eq!(status(&extracted), 0);
    assert_eq!(
        std::fs::read(into.join("KRX/OCD/Payload/ID-1/a.txt")).expect("the attachment"),
        b"alpha",
        "the bytes come back exactly as they went in"
    );
}

#[test]
fn stdout_writes_the_same_bytes_and_keeps_the_report_off_the_pipe() {
    let scratch = Scratch::new("create-stdout");
    let manifest = scene(
        &scratch,
        "{\"path\":\"a.txt\",\"description\":\"first\"}",
        &[("a.txt", b"alpha")],
    );
    let out = scratch.path().join("package.krx");
    assert_eq!(status(&create_json(&manifest, &out)), 0);

    let piped = run(&[
        "create",
        "--manifest",
        manifest.to_str().expect("a UTF-8 manifest path"),
        "--stdout",
        "--json",
    ]);
    assert_eq!(status(&piped), 0);
    assert_eq!(
        piped.stdout,
        std::fs::read(&out).expect("the package file"),
        "--stdout and --out produce the same package"
    );
    // In JSON mode with --stdout the single object is on stderr, so that the
    // package alone reaches the pipe.
    let response: serde_json::Value =
        serde_json::from_str(&stderr(&piped)).expect("one JSON object on stderr");
    assert_eq!(response["command"], "create");
    assert_eq!(response["data"]["entries"], 3);

    let human = run(&[
        "create",
        "--manifest",
        manifest.to_str().expect("a UTF-8 manifest path"),
        "--stdout",
    ]);
    assert_eq!(status(&human), 0);
    assert_eq!(human.stdout, piped.stdout);
    let text = stderr(&human);
    assert!(text.contains("went to standard output"), "{text}");
    assert!(text.contains("Nothing is verified"), "{text}");
}

#[test]
fn a_failed_stdout_run_keeps_its_report_off_the_package_pipe() {
    // stdout is the package pipe in --stdout mode, so a caller reading it must
    // find a package or nothing at all. A JSON envelope written there would
    // hand the next process in the pipeline a diagnostic as if it were bytes
    // of a package.
    let scratch = Scratch::new("create-stdout-failure");
    let manifest_path = scene(
        &scratch,
        "{\"path\":\"present.txt\"},{\"path\":\"absent.txt\"}",
        &[("present.txt", b"alpha")],
    );
    let manifest_argument = manifest_path.to_str().expect("a UTF-8 manifest path");

    let json = run(&[
        "create",
        "--manifest",
        manifest_argument,
        "--stdout",
        "--json",
    ]);
    assert_eq!(status(&json), 5);
    assert!(
        json.stdout.is_empty(),
        "stdout is the package pipe and must stay empty: {:?}",
        stdout(&json)
    );
    let text = stderr(&json);
    assert_eq!(
        text.lines().count(),
        1,
        "exactly one JSON object on stderr, so a caller can parse it whole: {text}"
    );
    let response: serde_json::Value =
        serde_json::from_str(&text).expect("one JSON object on stderr");
    assert_eq!(response["command"], "create");
    assert_eq!(response["ok"], false);
    assert_eq!(response["error"]["code"], "input.unreadable");
    assert_eq!(response["error"]["attachment_index"], 1);
    assert_eq!(response["cleanup"]["removed"], 0);

    // Human mode keeps the same rule: the diagnostic and the cleanup line are
    // on stderr, and nothing reaches the pipe.
    let human = run(&["create", "--manifest", manifest_argument, "--stdout"]);
    assert_eq!(status(&human), 5);
    assert!(human.stdout.is_empty(), "stdout must stay empty");
    let lines = stderr(&human);
    assert!(lines.contains("input.unreadable"), "{lines}");
    assert!(lines.contains("nothing had been written"), "{lines}");
}

#[test]
fn the_human_report_says_what_was_written_and_what_stays_undecided() {
    let scratch = Scratch::new("create-human");
    let manifest = scene(
        &scratch,
        "{\"path\":\"a.txt\",\"description\":\"first\"}",
        &[("a.txt", b"alpha")],
    );
    let out = scratch.path().join("package.krx");
    let output = create(&manifest, &out);
    assert_eq!(status(&output), 0);
    let text = stdout(&output);
    assert!(text.contains("in 3 entries"), "{text}");
    assert!(text.contains("canonical-documented"), "{text}");
    assert!(
        text.contains("validate-structure over this package exits 4"),
        "the report says what the package's own exit status will be: {text}"
    );
    assert!(text.contains("Nothing was overwritten"), "{text}");
    assert!(text.contains("Nothing is verified"), "{text}");
    assert!(
        !text.contains(&out.display().to_string()),
        "the report never echoes the path the caller gave: {text}"
    );
}

#[test]
fn an_unknown_key_is_refused_rather_than_ignored() {
    // The failure mode this refusal exists for: a misspelled `attachments`
    // would otherwise produce a package with no attachment, and a success
    // report saying so in numbers nobody reads.
    let scratch = Scratch::new("create-unknown-key");
    let text = manifest("").replace("\"attachments\"", "\"atachments\"");
    let manifest_path = scratch.file("manifest.json", text.as_bytes());
    let out = scratch.path().join("package.krx");
    let output = create_json(&manifest_path, &out);
    assert_eq!(status(&output), 6);
    let error = diagnostic(&output);
    assert_eq!(error["code"], "manifest.invalid.unknown_field");
    assert_eq!(error["category"], "package");
    assert_eq!(error["field"], "/", "the object, never the key itself");
    assert!(
        !stderr(&output).contains("atachments"),
        "the misspelled key is the caller's text and is never echoed"
    );
    assert!(!out.exists(), "a refused manifest writes nothing");
}

#[test]
fn each_manifest_defect_names_the_field_it_concerns() {
    let scratch = Scratch::new("create-manifest-defects");
    let base = manifest("");
    // Each case rewrites one part of the shared manifest, so the difference
    // between the accepted document and the refused one is exactly the defect.
    let cases: [(&str, String, &str, &str); 8] = [
        (
            "syntax",
            "{\"schema_version\": 1,".to_owned(),
            "manifest.invalid.syntax",
            "/",
        ),
        (
            "not an object",
            "[]".to_owned(),
            "manifest.invalid.syntax",
            "/",
        ),
        (
            "schema version",
            base.replace("\"schema_version\":1", "\"schema_version\":2"),
            "manifest.invalid.schema_version",
            "/schema_version",
        ),
        (
            "schema version type",
            base.replace("\"schema_version\":1", "\"schema_version\":\"1\""),
            "manifest.invalid.type",
            "/schema_version",
        ),
        (
            "missing field",
            base.replace("\"version\":\"0.9\",", ""),
            "manifest.invalid.missing_field",
            "/metadata/version",
        ),
        (
            "wrong type",
            base.replace("\"test\":true", "\"test\":\"true\""),
            "manifest.invalid.type",
            "/metadata/test",
        ),
        (
            "enumeration",
            base.replace("\"source_system\":\"KER\"", "\"source_system\":\"ker\""),
            "manifest.invalid.enumeration",
            "/metadata/source_system",
        ),
        (
            "timestamp",
            base.replace(MANIFEST_TIMESTAMP, "2026-01-02 03:04:06"),
            "manifest.invalid.timestamp",
            "/timestamp",
        ),
    ];
    for (label, text, code, field) in cases {
        let manifest_path = scratch.file(&format!("{label}.json"), text.as_bytes());
        let out = scratch.path().join(format!("{label}.krx"));
        let output = create_json(&manifest_path, &out);
        assert_eq!(status(&output), 6, "{label}");
        let error = diagnostic(&output);
        assert_eq!(error["code"], code, "{label}");
        assert_eq!(error["field"], field, "{label}");
        assert!(!out.exists(), "{label} wrote a package");
    }
}

#[test]
fn a_defect_inside_an_attachment_names_its_position() {
    let scratch = Scratch::new("create-attachment-defect");
    let manifest_path = scene(
        &scratch,
        "{\"path\":\"a.txt\"},{\"path\":\"b.txt\",\"describtion\":\"typo\"}",
        &[("a.txt", b"alpha"), ("b.txt", b"beta")],
    );
    let out = scratch.path().join("package.krx");
    let output = create_json(&manifest_path, &out);
    assert_eq!(status(&output), 6);
    let error = diagnostic(&output);
    assert_eq!(error["code"], "manifest.invalid.unknown_field");
    assert_eq!(error["field"], "/attachments");
    assert_eq!(error["attachment_index"], 1, "the second element");
    assert!(stderr(&output).contains("(attachment 1)"));
}

#[test]
fn an_attachment_that_cannot_be_read_is_an_input_failure() {
    let scratch = Scratch::new("create-missing-attachment");
    let manifest_path = scene(
        &scratch,
        "{\"path\":\"present.txt\"},{\"path\":\"absent.txt\"}",
        &[("present.txt", b"alpha")],
    );
    let out = scratch.path().join("package.krx");
    let output = create_json(&manifest_path, &out);
    assert_eq!(status(&output), 5);
    let error = diagnostic(&output);
    assert_eq!(error["code"], "input.unreadable");
    assert_eq!(error["category"], "input");
    assert_eq!(error["attachment_index"], 1);
    assert!(
        !stderr(&output).contains("absent.txt"),
        "the path the manifest named is never echoed"
    );
    assert!(!out.exists(), "nothing is written when a file is missing");
}

#[test]
fn a_manifest_that_cannot_be_read_is_an_input_failure() {
    let scratch = Scratch::new("create-missing-manifest");
    let missing = scratch.path().join("no-such-manifest.json");
    let out = scratch.path().join("package.krx");
    let output = create_json(&missing, &out);
    assert_eq!(status(&output), 5);
    assert_eq!(diagnostic(&output)["code"], "input.unreadable");
    assert!(!out.exists());
}

#[test]
fn a_name_the_writer_refuses_stops_the_run_before_anything_is_written() {
    let scratch = Scratch::new("create-unsafe-name");
    let cases = [
        ("KRX/escape.txt", "create.unsafe_name.separator"),
        ("..", "create.unsafe_name.parent_component"),
        ("CON", "create.unsafe_name.reserved_device_name"),
    ];
    for (index, (file_name, code)) in cases.iter().enumerate() {
        let manifest_path = scene(
            &scratch,
            &format!("{{\"path\":\"a.txt\",\"file_name\":\"{file_name}\"}}"),
            &[("a.txt", b"alpha")],
        );
        let out = scratch.path().join(format!("unsafe-{index}.krx"));
        let output = create_json(&manifest_path, &out);
        assert_eq!(status(&output), 6, "{file_name}");
        let error = diagnostic(&output);
        assert_eq!(error["code"], *code, "{file_name}");
        assert_eq!(error["attachment_index"], 0);
        assert!(!out.exists(), "{file_name} wrote a package");
    }
}

#[test]
fn a_count_that_disagrees_with_the_attachments_is_refused() {
    // A manifest may assert `declared_attachment_count`; the writer derives it
    // and refuses rather than writing a document describing a different
    // package than the one that comes out.
    let scratch = Scratch::new("create-count-mismatch");
    let text = manifest("{\"path\":\"a.txt\"}").replace(
        "\"test\":true",
        "\"test\":true,\"dispatches\":[{\"declared_attachment_count\":7}]",
    );
    let _ = scratch.file("a.txt", b"alpha");
    let manifest_path = scratch.file("manifest.json", text.as_bytes());
    let out = scratch.path().join("package.krx");
    let output = create_json(&manifest_path, &out);
    assert_eq!(status(&output), 6);
    assert_eq!(
        diagnostic(&output)["code"],
        "create.invalid.reference_mismatch"
    );
    assert!(!out.exists());
}

#[test]
fn a_count_that_agrees_with_the_attachments_is_accepted() {
    let scratch = Scratch::new("create-count-agrees");
    let text = manifest("{\"path\":\"a.txt\"}").replace(
        "\"test\":true",
        "\"test\":true,\"dispatches\":[{\"declared_attachment_count\":1}]",
    );
    let _ = scratch.file("a.txt", b"alpha");
    let manifest_path = scratch.file("manifest.json", text.as_bytes());
    let out = scratch.path().join("package.krx");
    assert_eq!(status(&create_json(&manifest_path, &out)), 0);
    assert_eq!(validate(&out).0, 4);
}

#[test]
fn an_output_that_already_exists_is_refused_and_left_alone() {
    let scratch = Scratch::new("create-no-clobber");
    let manifest_path = scene(&scratch, "", &[]);
    let out = scratch.file("package.krx", b"someone else's file");
    let output = create_json(&manifest_path, &out);
    assert_eq!(status(&output), 9);
    let error = diagnostic(&output);
    assert_eq!(error["code"], "output.exists");
    assert_eq!(error["category"], "output");
    assert_eq!(
        one_object(&output)["cleanup"],
        serde_json::json!({"removed": 0, "left_in_place": 0}),
        "nothing had been written, so nothing was undone"
    );
    assert_eq!(
        std::fs::read(&out).expect("the existing file"),
        b"someone else's file",
        "the file that was there is untouched"
    );
}

#[test]
fn an_output_directory_that_is_missing_or_not_a_directory_is_refused() {
    let scratch = Scratch::new("create-output-parent");
    let manifest_path = scene(&scratch, "", &[]);
    let missing = scratch.path().join("no-such-directory").join("package.krx");
    let output = create_json(&manifest_path, &missing);
    assert_eq!(status(&output), 9);
    assert_eq!(diagnostic(&output)["code"], "output.destination_missing");

    let file = scratch.file("a-file", b"not a directory");
    let through = file.join("package.krx");
    let output = create_json(&manifest_path, &through);
    assert_eq!(status(&output), 9);
    assert_eq!(
        diagnostic(&output)["code"],
        "output.destination_not_a_directory"
    );
}

#[cfg(unix)]
#[test]
fn an_output_directory_that_is_a_symbolic_link_is_refused() {
    let scratch = Scratch::new("create-output-symlink");
    let manifest_path = scene(&scratch, "", &[]);
    let real = scratch.dir("real");
    let link = scratch.path().join("link");
    std::os::unix::fs::symlink(&real, &link).expect("create a symbolic link");
    let output = create_json(&manifest_path, &link.join("package.krx"));
    assert_eq!(status(&output), 9);
    assert_eq!(diagnostic(&output)["code"], "output.destination_symlink");
    assert!(
        !real.join("package.krx").exists(),
        "writing through the link would have left the destination the caller \
did not name"
    );
}

#[cfg(unix)]
#[test]
fn an_output_that_is_a_dangling_symbolic_link_counts_as_occupied() {
    let scratch = Scratch::new("create-dangling-out");
    let manifest_path = scene(&scratch, "", &[]);
    let out = scratch.path().join("package.krx");
    std::os::unix::fs::symlink(scratch.path().join("nowhere"), &out)
        .expect("create a dangling symbolic link");
    let output = create_json(&manifest_path, &out);
    assert_eq!(status(&output), 9);
    assert_eq!(diagnostic(&output)["code"], "output.exists");
}

/// The two output-link rules on Windows, against a directory junction.
///
/// `--out` names one file in one directory, so the destination *is* the only
/// ancestor `crate::output::parent` examines: a junction there and a junction
/// at the path itself are the whole rule on this platform, and they are what
/// exercises `FILE_ATTRIBUTE_REPARSE_POINT` in
/// `crate::extract::preflight::is_link` for the writing commands. `mklink /J`
/// needs no elevation; a runner without it leaves a `SKIPPED:` line and the
/// test returns.
#[cfg(windows)]
mod junctions {
    use super::{Scratch, create_json, diagnostic, scene, status};
    use crate::support::junction;

    #[test]
    fn an_output_directory_that_is_a_junction_is_refused() {
        let scratch = Scratch::new("create-output-junction");
        let manifest_path = scene(&scratch, "", &[]);
        let real = scratch.dir("real");
        let link = scratch.path().join("link");
        if !junction(&link, &real) {
            return;
        }
        let output = create_json(&manifest_path, &link.join("package.krx"));
        assert_eq!(status(&output), 9);
        assert_eq!(diagnostic(&output)["code"], "output.destination_symlink");
        assert!(
            !real.join("package.krx").exists(),
            "writing through the junction would have left the destination the \
caller did not name"
        );
    }

    #[test]
    fn an_output_that_is_a_junction_counts_as_occupied() {
        let scratch = Scratch::new("create-junction-out");
        let manifest_path = scene(&scratch, "", &[]);
        let target = scratch.dir("target");
        let out = scratch.path().join("package.krx");
        if !junction(&out, &target) {
            return;
        }
        let output = create_json(&manifest_path, &out);
        assert_eq!(status(&output), 9);
        assert_eq!(diagnostic(&output)["code"], "output.exists");
        assert_eq!(
            std::fs::read_dir(&target)
                .expect("read the junction target")
                .count(),
            0,
            "the junction's target was never written through"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_destination_that_cannot_be_written_reports_an_output_failure() {
    use std::os::unix::fs::PermissionsExt;

    let scratch = Scratch::new("create-readonly");
    let manifest_path = scene(&scratch, "", &[]);
    let directory = scratch.dir("read-only");
    let mut permissions = std::fs::metadata(&directory)
        .expect("the destination directory")
        .permissions();
    permissions.set_mode(0o500);
    std::fs::set_permissions(&directory, permissions).expect("make it read-only");

    let out = directory.join("package.krx");
    let output = create_json(&manifest_path, &out);

    // Restore the mode first, so the scratch directory can remove itself
    // whatever the assertions below do.
    let mut restored = std::fs::metadata(&directory)
        .expect("the destination directory")
        .permissions();
    restored.set_mode(0o700);
    std::fs::set_permissions(&directory, restored).expect("restore the mode");

    assert_eq!(status(&output), 9);
    assert_eq!(diagnostic(&output)["code"], "output.io");
    assert!(!out.exists(), "nothing was left behind");
}

#[test]
fn no_manifest_value_reaches_a_diagnostic() {
    // Canaries in the three places a manifest carries caller text: a metadata
    // value, a package-internal file name, and a path on the caller's own
    // filesystem. A refusal must carry none of them, on either stream.
    let scratch = Scratch::new("create-canaries");
    let directory = scratch.dir(CANARY_PATH);
    std::fs::write(directory.join("a.txt"), b"alpha").expect("write the attachment");
    let text = manifest(&format!(
        "{{\"path\":\"{CANARY_PATH}/a.txt\",\"file_name\":\"{CANARY_ENTRY}\",\
\"describtion\":\"a misspelled key\"}}"
    ))
    .replace("SYNTHETIC-CONSIGNMENT-1", CANARY_VALUE);
    let manifest_path = scratch.file("manifest.json", text.as_bytes());
    let out = scratch.path().join("package.krx");
    let output = create_json(&manifest_path, &out);
    assert_eq!(status(&output), 6);
    let streams = format!("{}{}", stdout(&output), stderr(&output));
    for canary in [CANARY_PATH, CANARY_ENTRY, CANARY_VALUE] {
        assert!(
            !streams.contains(canary),
            "a diagnostic carried {canary}:\n{streams}"
        );
    }
}

#[test]
fn a_successful_report_carries_no_value_from_the_manifest_either() {
    let scratch = Scratch::new("create-canaries-success");
    let directory = scratch.dir(CANARY_PATH);
    std::fs::write(directory.join("a.txt"), b"alpha").expect("write the attachment");
    let text = manifest(&format!(
        "{{\"path\":\"{CANARY_PATH}/a.txt\",\"file_name\":\"{CANARY_ENTRY}\",\
\"description\":\"a described attachment\"}}"
    ))
    .replace("SYNTHETIC-CONSIGNMENT-1", CANARY_VALUE);
    let manifest_path = scratch.file("manifest.json", text.as_bytes());
    let out = scratch.path().join("package.krx");
    let output = create_json(&manifest_path, &out);
    assert_eq!(status(&output), 0);
    // The report counts and cites rules; it names nothing from the package.
    // `inspect` is where a declared value is printed, on request.
    let streams = format!("{}{}", stdout(&output), stderr(&output));
    for canary in [CANARY_PATH, CANARY_ENTRY, CANARY_VALUE] {
        assert!(
            !streams.contains(canary),
            "the report carried {canary}:\n{streams}"
        );
    }
}

#[test]
fn every_optional_metadata_field_reaches_the_document() {
    let scratch = Scratch::new("create-optional");
    let text = manifest("{\"path\":\"a.txt\",\"file_name\":null,\"description\":null}").replace(
        "\"test\":true",
        "\"test\":false,\"barcode\":\"SYNTHETIC-BARCODE\",\
\"reference_id\":\"SYNTHETIC-REFERENCE\",\"error_code\":\"SYNTHETIC-ERROR\",\
\"note\":\"a synthetic note\",\"dispatches\":null",
    );
    let _ = scratch.file("a.txt", b"alpha");
    let manifest_path = scratch.file("manifest.json", text.as_bytes());
    let out = scratch.path().join("package.krx");
    assert_eq!(status(&create_json(&manifest_path, &out)), 0);

    let inspected = one_object(&run(&[
        "inspect",
        out.to_str().expect("a UTF-8 package path"),
        "--json",
    ]));
    let metadata = &inspected["data"]["metadata"];
    assert_eq!(metadata["barcode"], "SYNTHETIC-BARCODE");
    assert_eq!(metadata["reference_id"], "SYNTHETIC-REFERENCE");
    assert_eq!(metadata["error_code"], "SYNTHETIC-ERROR");
    assert_eq!(metadata["test"], false);
    assert_eq!(
        metadata["attachments"][0]["declared_path"], "KRX/OCD/Payload/ID-1/a.txt",
        "an absent file_name defaults to the path's last component"
    );
    // An attachment with no description leaves M11 undecided, beside the two
    // rules every written package cites.
    let (code, report) = validate(&out);
    assert_eq!(code, 4);
    assert_eq!(
        report["data"]["unresolved_rules"],
        serde_json::json!(["A19", "M11", "M13"])
    );
}

#[test]
fn a_value_of_the_wrong_shape_is_refused_wherever_it_sits() {
    let scratch = Scratch::new("create-shapes");
    let base = manifest("{\"path\":\"a.txt\"}");
    let _ = scratch.file("a.txt", b"alpha");
    let cases: [(&str, String, &str, &str); 7] = [
        (
            "metadata not an object",
            base.replace("\"metadata\":{", "\"metadata\":[{"),
            "manifest.invalid.syntax",
            "/",
        ),
        (
            "attachments not an array",
            base.replace("\"attachments\":[", "\"attachments\":{\"0\":["),
            "manifest.invalid.syntax",
            "/",
        ),
        (
            "an attachment that is not an object",
            manifest("\"a.txt\""),
            "manifest.invalid.type",
            "/attachments",
        ),
        (
            "an attachment with no path",
            manifest("{\"description\":\"no path at all\"}"),
            "manifest.invalid.missing_field",
            "/attachments/path",
        ),
        (
            "a file name of the wrong type",
            manifest("{\"path\":\"a.txt\",\"file_name\":7}"),
            "manifest.invalid.type",
            "/attachments/file_name",
        ),
        (
            "dispatches not an array",
            base.replace("\"test\":true", "\"test\":true,\"dispatches\":{}"),
            "manifest.invalid.type",
            "/metadata/dispatches",
        ),
        (
            "a declared count of the wrong type",
            base.replace(
                "\"test\":true",
                "\"test\":true,\"dispatches\":[{\"declared_attachment_count\":\"1\"}]",
            ),
            "manifest.invalid.type",
            "/metadata/dispatches/declared_attachment_count",
        ),
    ];
    for (index, (label, text, code, field)) in cases.into_iter().enumerate() {
        let manifest_path = scratch.file(&format!("shape-{index}.json"), text.as_bytes());
        let out = scratch.path().join(format!("shape-{index}.krx"));
        let output = create_json(&manifest_path, &out);
        assert_eq!(status(&output), 6, "{label}");
        let error = diagnostic(&output);
        assert_eq!(error["code"], code, "{label}");
        assert_eq!(error["field"], field, "{label}");
        assert!(!out.exists(), "{label} wrote a package");
    }
}

#[test]
fn a_dispatch_block_is_written_even_with_no_attachment_to_list() {
    // An explicit dispatch array is honoured as written: the writer derives
    // the count into it, and the package still reads back cleanly.
    let scratch = Scratch::new("create-empty-dispatch");
    let text = manifest("").replace("\"test\":true", "\"test\":true,\"dispatches\":[{}]");
    let manifest_path = scratch.file("manifest.json", text.as_bytes());
    let out = scratch.path().join("package.krx");
    assert_eq!(status(&create_json(&manifest_path, &out)), 0);
    let (code, report) = validate(&out);
    assert_eq!(code, 4);
    assert_eq!(report["data"]["summary"], "unresolved");
}

#[test]
fn every_timestamp_the_record_cannot_hold_is_refused() {
    let scratch = Scratch::new("create-timestamps");
    for (index, stamp) in [
        "2026-01-02T03:04",     // too short
        "2026-01-02T03:04:06Z", // too long
        "2026-01-02t03:04:06",  // the wrong separator
        "2026-01-0aT03:04:06",  // a non-digit
        "1979-12-31T23:59:58",  // before the MS-DOS epoch
        "2108-01-01T00:00:00",  // past what the field can express
        "2026-02-30T00:00:00",  // a day that does not exist
    ]
    .into_iter()
    .enumerate()
    {
        let text = manifest("").replace(MANIFEST_TIMESTAMP, stamp);
        let manifest_path = scratch.file(&format!("stamp-{index}.json"), text.as_bytes());
        let out = scratch.path().join(format!("stamp-{index}.krx"));
        let output = create_json(&manifest_path, &out);
        assert_eq!(status(&output), 6, "{stamp}");
        let error = diagnostic(&output);
        assert_eq!(error["code"], "manifest.invalid.timestamp", "{stamp}");
        assert_eq!(error["field"], "/timestamp", "{stamp}");
        assert!(!out.exists(), "{stamp} wrote a package");
    }
}

#[test]
fn a_bare_output_name_is_written_in_the_working_directory() {
    // `--out package.krx` has no parent component: the file goes in the
    // directory the process is already in, and the run must not read that as
    // a missing destination.
    let scratch = Scratch::new("create-bare-out");
    let _ = scene(&scratch, "", &[]);
    let output = support::run_in(
        scratch.path(),
        &[
            "create",
            "--manifest",
            "manifest.json",
            "--out",
            "package.krx",
        ],
    );
    assert_eq!(status(&output), 0, "{}", stderr(&output));
    assert!(scratch.path().join("package.krx").is_file());
}

#[test]
fn a_manifest_that_names_no_file_name_and_has_no_last_component_is_refused() {
    let scratch = Scratch::new("create-nameless");
    let manifest_path = scene(&scratch, "{\"path\":\"a.txt/..\"}", &[("a.txt", b"alpha")]);
    let out = scratch.path().join("package.krx");
    let output = create_json(&manifest_path, &out);
    // The path does not resolve to a readable file on any supported system,
    // and if it ever did, the empty file name would be refused by the writer.
    assert!(
        [5, 6].contains(&status(&output)),
        "an input or a name refusal, not a written package"
    );
    assert!(!out.exists());
}

#[test]
fn the_create_help_states_the_manifest_rule_and_the_exit_four_outcome() {
    let text = stdout(&run(&["create", "--help"]));
    assert!(text.contains("--manifest"), "{text}");
    assert!(text.contains("--out"), "{text}");
    assert!(text.contains("--stdout"), "{text}");
    assert!(text.contains("YYYY-MM-DDTHH:MM:SS"), "{text}");
    assert!(
        text.contains("refused rather than ignored"),
        "the strictness of the manifest is discoverable: {text}"
    );
    assert!(
        text.contains("validate-structure over it exits 4"),
        "a caller must not read the 4 as a defect: {text}"
    );
    assert!(text.contains("nothing is verified"), "{text}");
}
