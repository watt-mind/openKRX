//! `repack`: what it preserves, what it changes, and everything it refuses.
//!
//! Every test runs the executable as a subprocess over a package the
//! executable itself created, because that is the contract a caller has: a
//! package and a JSON document go in, and either a new package and a report
//! come out, or nothing is written at all.
//!
//! Four properties carry the most weight and each has its own test:
//!
//! - **Preservation.** An attachment no edit names comes out of the new
//!   package byte-identical to the one in the old, compared by extracting
//!   both and reading the files.
//! - **Determinism and identity.** Repacking with no edit rewrites the same
//!   package byte for byte, and repacking the result again changes nothing.
//! - **Refusal without loss.** A package carrying something the writer cannot
//!   re-emit is refused whole, with exit `7`, and nothing is written.
//! - **No clobbering.** `--out` must not exist in any form, so the package
//!   being edited can never be overwritten by the command editing it.
mod support;

use std::path::{Path, PathBuf};

use support::{
    CANARY_ENTRY, CANARY_PATH, CANARY_VALUE, Scratch, manifest, one_object, run, run_stdin, status,
    stderr, stdout,
};

/// The timestamp every edits document in this file carries.
///
/// It is the source manifest's own, so that repacking with no edit must
/// reproduce the package `create` wrote byte for byte: openKRX has no clock,
/// and the timestamp is the only value that could otherwise differ.
const STAMP: &str = support::MANIFEST_TIMESTAMP;

/// The attachments the source package carries.
const ATTACHMENTS: &str = "{\"path\":\"a.txt\",\"description\":\"first\"},\
{\"path\":\"b.txt\",\"file_name\":\"renamed-b.txt\",\"description\":\"second\"}";

/// The bytes of the two attachment files.
const ALPHA: &[u8] = b"alpha bytes\n";
const BETA: &[u8] = b"beta bytes\n";

/// A path as an argument; every path a test builds is UTF-8.
fn text(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 path")
}

/// Write the manifest and its files, and create the package to be edited.
fn source(scratch: &Scratch) -> PathBuf {
    let _ = scratch.file("a.txt", ALPHA);
    let _ = scratch.file("b.txt", BETA);
    let manifest = scratch.file("manifest.json", manifest(ATTACHMENTS).as_bytes());
    let package = scratch.path().join("source.krx");
    let output = run(&[
        "create",
        "--manifest",
        text(&manifest),
        "--out",
        text(&package),
    ]);
    assert_eq!(status(&output), 0, "the source package is created");
    package
}

/// One edits document, with `body` spliced in after the fixed two fields.
fn edits(body: &str) -> String {
    let separator = if body.is_empty() { "" } else { "," };
    format!("{{\"schema_version\":1,\"timestamp\":\"{STAMP}\"{separator}{body}}}")
}

/// Write an edits document into the scratch directory.
fn edits_file(scratch: &Scratch, body: &str) -> PathBuf {
    scratch.file("edits.json", edits(body).as_bytes())
}

/// Run `repack` in JSON mode.
fn repack_json(package: &Path, edits: &Path, out: &Path) -> std::process::Output {
    run(&[
        "repack",
        text(package),
        "--edits",
        text(edits),
        "--out",
        text(out),
        "--json",
    ])
}

/// The same, in human mode.
fn repack(package: &Path, edits: &Path, out: &Path) -> std::process::Output {
    run(&[
        "repack",
        text(package),
        "--edits",
        text(edits),
        "--out",
        text(out),
    ])
}

/// The `error` object of a refusal.
fn diagnostic(output: &std::process::Output) -> serde_json::Value {
    one_object(output)
        .get("error")
        .cloned()
        .expect("a failed envelope carries error")
}

/// Extract `package` into a fresh directory and return it.
fn extract(scratch: &Scratch, package: &Path, into: &str) -> PathBuf {
    let directory = scratch.dir(into);
    let output = run(&["extract", text(package), "--into", text(&directory)]);
    assert_eq!(status(&output), 0, "the package extracts");
    directory
}

// ------------------------------------------------------------- preservation

#[test]
fn an_attachment_no_edit_names_comes_out_byte_identical() {
    // The whole point of the command, asserted on the bytes on disk rather
    // than on anything openkrx reports about them.
    let scratch = Scratch::new("repack-preserve");
    let package = source(&scratch);
    let edits = edits_file(
        &scratch,
        "\"metadata\":{\"consignment_id\":\"EDITED\"},\"remove\":[1]",
    );
    let out = scratch.path().join("out.krx");
    assert_eq!(status(&repack_json(&package, &edits, &out)), 0);

    let before = extract(&scratch, &package, "before");
    let after = extract(&scratch, &out, "after");
    // Attachment 2 becomes attachment 1, and its bytes do not change.
    let source_bytes =
        std::fs::read(before.join("KRX/OCD/Payload/ID-2/renamed-b.txt")).expect("the old file");
    let result_bytes =
        std::fs::read(after.join("KRX/OCD/Payload/ID-1/renamed-b.txt")).expect("the new file");
    assert_eq!(source_bytes, BETA);
    assert_eq!(result_bytes, source_bytes);
    // And the removed attachment is gone rather than emptied.
    assert!(!after.join("KRX/OCD/Payload/ID-2").exists());
}

#[test]
fn repacking_with_no_edit_rewrites_the_same_package_and_is_idempotent() {
    let scratch = Scratch::new("repack-identity");
    let package = source(&scratch);
    let edits = edits_file(&scratch, "");
    let once = scratch.path().join("once.krx");
    let twice = scratch.path().join("twice.krx");
    assert_eq!(status(&repack_json(&package, &edits, &once)), 0);
    assert_eq!(status(&repack_json(&once, &edits, &twice)), 0);
    assert_eq!(
        std::fs::read(&once).expect("the first result"),
        std::fs::read(&package).expect("the source package"),
        "an empty edit rewrites the package create wrote, byte for byte"
    );
    assert_eq!(
        std::fs::read(&once).expect("the first result"),
        std::fs::read(&twice).expect("the second result"),
        "repacking a repacked package changes nothing"
    );
}

#[test]
fn the_package_being_edited_is_never_written_to() {
    let scratch = Scratch::new("repack-untouched");
    let package = source(&scratch);
    let before = std::fs::read(&package).expect("the source package");
    let edits = edits_file(&scratch, "\"remove\":[1,2]");
    let out = scratch.path().join("out.krx");
    assert_eq!(status(&repack_json(&package, &edits, &out)), 0);
    assert_eq!(
        std::fs::read(&package).expect("the source package"),
        before,
        "the input is read, never rewritten"
    );
}

// ------------------------------------------------------------- the report

#[test]
fn the_report_says_what_changed_and_what_was_preserved() {
    let scratch = Scratch::new("repack-report");
    let package = source(&scratch);
    let _ = scratch.file("c.bin", b"gamma");
    let edits = edits_file(
        &scratch,
        "\"metadata\":{\"consignment_id\":\"EDITED\",\"note\":\"a note\"},\
\"replace\":[{\"number\":1,\"path\":\"c.bin\"}],\
\"add\":[{\"path\":\"c.bin\",\"file_name\":\"added.bin\",\"description\":\"the added one\"}]",
    );
    let out = scratch.path().join("out.krx");
    let output = repack_json(&package, &edits, &out);
    assert_eq!(status(&output), 0);
    assert!(output.stderr.is_empty(), "a successful run says nothing");
    let response = one_object(&output);
    assert_eq!(response["command"], "repack");
    assert_eq!(response["verified"], false);
    let data = &response["data"];
    assert_eq!(data["preserved"], serde_json::json!([2]));
    assert_eq!(data["changed"], serde_json::json!([1]));
    assert_eq!(data["added"], serde_json::json!([3]));
    assert_eq!(data["removed"], serde_json::json!([]));
    assert_eq!(
        data["header_fields"],
        serde_json::json!(["consignment_id", "note"])
    );
    assert_eq!(data["entries"], 5);
    assert_eq!(data["layout"], "canonical-documented");
    assert_eq!(data["unresolved_rules"], serde_json::json!(["A19", "M13"]));
    assert_eq!(
        data["bytes_written"].as_u64().expect("a byte count"),
        std::fs::metadata(&out).expect("the package exists").len(),
        "the report counts the bytes that are on disk"
    );
}

#[test]
fn the_human_report_distinguishes_the_two_numberings() {
    let scratch = Scratch::new("repack-human");
    let package = source(&scratch);
    let _ = scratch.file("c.bin", b"gamma");
    let edits = edits_file(&scratch, "\"remove\":[1],\"add\":[{\"path\":\"c.bin\"}]");
    let out = scratch.path().join("out.krx");
    let output = repack(&package, &edits, &out);
    assert_eq!(status(&output), 0);
    let text = stdout(&output);
    assert!(text.contains("In the package that was edited"), "{text}");
    assert!(text.contains("In the package that was written"), "{text}");
    assert!(text.contains("preserved byte for byte"), "{text}");
    assert!(text.contains("Nothing is verified."), "{text}");
    assert!(
        !text.contains("renamed-b.txt"),
        "the report names no file: {text}"
    );
}

#[test]
fn a_header_edit_is_visible_in_the_package_that_was_written() {
    let scratch = Scratch::new("repack-header");
    let package = source(&scratch);
    let edits = edits_file(
        &scratch,
        "\"metadata\":{\"consignment_id\":\"EDITED-CONSIGNMENT\",\"note\":\"why\"}",
    );
    let out = scratch.path().join("out.krx");
    assert_eq!(status(&repack_json(&package, &edits, &out)), 0);
    let inspected = one_object(&run(&["inspect", text(&out), "--json"]));
    let metadata = &inspected["data"]["metadata"];
    assert_eq!(metadata["consignment_id"], "EDITED-CONSIGNMENT");
    assert_eq!(metadata["source_system"], "KER", "untouched fields stay");
    assert_eq!(inspected["data"]["metadata"]["attachments"][0]["number"], 1);
}

#[test]
fn the_package_can_be_read_from_standard_input() {
    let scratch = Scratch::new("repack-stdin");
    let package = source(&scratch);
    let edits = edits_file(&scratch, "");
    let out = scratch.path().join("out.krx");
    let bytes = std::fs::read(&package).expect("the source package");
    let output = run_stdin(
        &[
            "repack",
            "-",
            "--edits",
            text(&edits),
            "--out",
            text(&out),
            "--json",
        ],
        &bytes,
    );
    assert_eq!(status(&output), 0);
    assert_eq!(std::fs::read(&out).expect("the result"), bytes);
}

#[test]
fn stdout_mode_writes_the_same_bytes_and_puts_the_report_on_stderr() {
    let scratch = Scratch::new("repack-stdout");
    let package = source(&scratch);
    let edits = edits_file(&scratch, "\"remove\":[2]");
    let out = scratch.path().join("out.krx");
    assert_eq!(status(&repack_json(&package, &edits, &out)), 0);
    let output = run(&[
        "repack",
        text(&package),
        "--edits",
        text(&edits),
        "--stdout",
        "--json",
    ]);
    assert_eq!(status(&output), 0);
    assert_eq!(
        output.stdout,
        std::fs::read(&out).expect("the file result"),
        "--stdout writes exactly what --out would have written"
    );
    let report: serde_json::Value =
        serde_json::from_str(stderr(&output).trim()).expect("one object on stderr");
    assert_eq!(report["command"], "repack");
    assert_eq!(report["data"]["removed"], serde_json::json!([2]));
}

// --------------------------------------------------------------- refusals

#[test]
fn a_package_openkrx_cannot_re_emit_is_refused_and_nothing_is_written() {
    let scratch = Scratch::new("repack-unsupported");
    // One of the other two layouts rule A19 describes: openkrx reads it, and
    // refuses to rewrite it into the layout it writes.
    let package = scratch.file("foreign.krx", &support::layout_package(""));
    let edits = edits_file(&scratch, "");
    let out = scratch.path().join("out.krx");
    let output = repack_json(&package, &edits, &out);
    assert_eq!(status(&output), 7);
    let error = diagnostic(&output);
    assert_eq!(error["code"], "repack.unsupported.root_prefix");
    assert_eq!(error["category"], "unsupported");
    assert!(!out.exists(), "a refused run writes nothing");
    // The sentence has to say the package is not damaged, or a caller will
    // read exit 7 as "my package is broken".
    let human = repack(&package, &edits, &out);
    assert!(stderr(&human).contains("not damaged"), "{}", stderr(&human));
}

#[test]
fn a_package_carrying_an_entry_the_layout_has_no_place_for_is_refused() {
    let scratch = Scratch::new("repack-extra");
    let package = scratch.file("extra.krx", &support::attachment_package());
    let edits = edits_file(&scratch, "");
    let out = scratch.path().join("out.krx");
    let output = repack_json(&package, &edits, &out);
    // The synthetic package's marker sits at the archive root, which is the
    // first thing that stops openkrx from re-emitting it.
    assert_eq!(status(&output), 7);
    assert!(
        diagnostic(&output)["code"]
            .as_str()
            .expect("a code")
            .starts_with("repack.unsupported."),
    );
}

#[test]
fn an_edit_naming_an_attachment_the_package_does_not_hold_is_refused() {
    let scratch = Scratch::new("repack-no-such");
    let package = source(&scratch);
    let edits = edits_file(&scratch, "\"remove\":[9]");
    let out = scratch.path().join("out.krx");
    let output = repack_json(&package, &edits, &out);
    assert_eq!(status(&output), 6);
    let error = diagnostic(&output);
    assert_eq!(error["code"], "repack.invalid.no_such_attachment");
    assert_eq!(error["attachment_number"], 9);
    assert!(!out.exists());
    assert_eq!(
        one_object(&output)["cleanup"],
        serde_json::json!({"removed": 0, "left_in_place": 0}),
        "nothing had been written"
    );
}

#[test]
fn two_edits_naming_the_same_attachment_are_refused() {
    let scratch = Scratch::new("repack-duplicate");
    let package = source(&scratch);
    let _ = scratch.file("c.bin", b"gamma");
    let edits = edits_file(
        &scratch,
        "\"remove\":[1],\"replace\":[{\"number\":1,\"path\":\"c.bin\"}]",
    );
    let out = scratch.path().join("out.krx");
    let output = repack_json(&package, &edits, &out);
    assert_eq!(status(&output), 6);
    assert_eq!(
        diagnostic(&output)["code"],
        "repack.invalid.duplicate_target"
    );
}

#[test]
fn each_edits_defect_names_the_field_it_concerns() {
    let scratch = Scratch::new("repack-schema");
    let package = source(&scratch);
    let out = scratch.path().join("out.krx");
    for (body, code, field) in [
        (
            "\"metadata\":{\"consignment_ip\":\"typo\"}".to_owned(),
            "manifest.invalid.unknown_field",
            "/metadata",
        ),
        (
            "\"metadata\":{\"source_system\":\"NOT_A_SYSTEM\"}".to_owned(),
            "manifest.invalid.enumeration",
            "/metadata/source_system",
        ),
        (
            "\"metadata\":{\"test\":\"true\"}".to_owned(),
            "manifest.invalid.type",
            "/metadata/test",
        ),
        (
            "\"metadata\":{\"dispatches\":[]}".to_owned(),
            "manifest.invalid.unknown_field",
            "/metadata",
        ),
        (
            "\"remove\":[\"1\"]".to_owned(),
            "manifest.invalid.type",
            "/remove",
        ),
        (
            "\"replace\":[{\"number\":1}]".to_owned(),
            "manifest.invalid.missing_field",
            "/replace/path",
        ),
        (
            "\"add\":[{\"path\":\"a.txt\",\"describtion\":\"typo\"}]".to_owned(),
            "manifest.invalid.unknown_field",
            "/add",
        ),
    ] {
        let edits = scratch.file("case.json", edits(&body).as_bytes());
        let output = repack_json(&package, &edits, &out);
        assert_eq!(status(&output), 6, "{body}");
        let error = diagnostic(&output);
        assert_eq!(error["code"], code, "{body}");
        assert_eq!(error["field"], field, "{body}");
        assert!(!out.exists(), "{body}");
    }
}

#[test]
fn an_edits_document_without_a_timestamp_is_refused() {
    // openkrx has no clock: there is no "now" to fall back on, so two runs
    // over the same edits cannot produce different bytes.
    let scratch = Scratch::new("repack-timestamp");
    let package = source(&scratch);
    let edits = scratch.file("edits.json", b"{\"schema_version\":1}");
    let out = scratch.path().join("out.krx");
    let output = repack_json(&package, &edits, &out);
    assert_eq!(status(&output), 6);
    let error = diagnostic(&output);
    assert_eq!(error["code"], "manifest.invalid.missing_field");
    assert_eq!(error["field"], "/timestamp");
}

#[test]
fn an_edits_document_of_another_schema_version_is_refused() {
    let scratch = Scratch::new("repack-version");
    let package = source(&scratch);
    let edits = scratch.file(
        "edits.json",
        format!("{{\"schema_version\":2,\"timestamp\":\"{STAMP}\"}}").as_bytes(),
    );
    let out = scratch.path().join("out.krx");
    let output = repack_json(&package, &edits, &out);
    assert_eq!(status(&output), 6);
    assert_eq!(
        diagnostic(&output)["code"],
        "manifest.invalid.schema_version"
    );
}

#[test]
fn a_file_an_edit_names_that_cannot_be_read_is_an_input_failure() {
    let scratch = Scratch::new("repack-missing-file");
    let package = source(&scratch);
    let edits = edits_file(
        &scratch,
        "\"add\":[{\"path\":\"a.txt\"},{\"path\":\"no-such-file.bin\"}]",
    );
    let out = scratch.path().join("out.krx");
    let output = repack_json(&package, &edits, &out);
    assert_eq!(status(&output), 5);
    let error = diagnostic(&output);
    assert_eq!(error["code"], "input.unreadable");
    assert_eq!(error["attachment_index"], 1, "the second add element");
    assert!(!out.exists());
}

#[test]
fn an_output_that_already_exists_is_refused_and_left_alone() {
    let scratch = Scratch::new("repack-clobber");
    let package = source(&scratch);
    let edits = edits_file(&scratch, "");
    let occupied = scratch.file("out.krx", b"someone else's file");
    let output = repack_json(&package, &edits, &occupied);
    assert_eq!(status(&output), 9);
    assert_eq!(diagnostic(&output)["code"], "output.exists");
    assert_eq!(
        std::fs::read(&occupied).expect("the occupied file"),
        b"someone else's file",
        "nothing is ever overwritten"
    );
}

#[test]
fn repacking_onto_the_package_being_edited_is_refused() {
    // The obvious in-place edit a caller will try. It must be refused before
    // anything is written, and the package must survive unchanged.
    let scratch = Scratch::new("repack-in-place");
    let package = source(&scratch);
    let before = std::fs::read(&package).expect("the source package");
    let edits = edits_file(&scratch, "\"remove\":[1]");
    let output = repack_json(&package, &edits, &package);
    assert_eq!(status(&output), 9);
    assert_eq!(diagnostic(&output)["code"], "output.exists");
    assert_eq!(std::fs::read(&package).expect("the package"), before);
    assert!(
        stderr(&output).contains("never overwritten") || !stderr(&output).is_empty(),
        "the refusal explains itself"
    );
}

#[test]
fn an_unreadable_package_is_an_input_failure_and_a_broken_one_a_package_failure() {
    let scratch = Scratch::new("repack-input");
    let edits = edits_file(&scratch, "");
    let out = scratch.path().join("out.krx");
    let missing = scratch.path().join("no-such-package.krx");
    assert_eq!(status(&repack_json(&missing, &edits, &out)), 5);

    let broken = scratch.file("broken.krx", &support::malformed_image());
    let output = repack_json(&broken, &edits, &out);
    assert_eq!(status(&output), 6);
    assert_eq!(
        diagnostic(&output)["code"],
        "archive.malformed.eocd_missing"
    );
}

#[test]
fn missing_arguments_are_usage_errors_with_no_envelope() {
    let scratch = Scratch::new("repack-usage");
    let package = source(&scratch);
    let edits = edits_file(&scratch, "");
    for arguments in [
        vec!["repack", "--edits", text(&edits), "--out", "x.krx"],
        vec!["repack", text(&package), "--out", "x.krx"],
        vec!["repack", text(&package), "--edits", text(&edits)],
    ] {
        let output = run(&arguments);
        assert_eq!(status(&output), 2, "{arguments:?}");
        assert!(output.stdout.is_empty(), "{arguments:?}");
    }
}

// ---------------------------------------------------------------- privacy

#[test]
fn no_diagnostic_carries_anything_the_edits_or_the_package_said() {
    // The values a caller writes into an edits document are package content,
    // exactly as a manifest's are, and a refusal must stay safe to log.
    let scratch = Scratch::new("repack-canary");
    let package = source(&scratch);
    let body = format!(
        "\"metadata\":{{\"consignment_id\":\"{CANARY_VALUE}\"}},\
\"remove\":[9],\"add\":[{{\"path\":\"{CANARY_PATH}/{CANARY_ENTRY}\"}}]"
    );
    let edits = scratch.file("edits.json", edits(&body).as_bytes());
    let out = scratch.path().join("out.krx");
    for arguments in [
        vec![
            "repack",
            text(&package),
            "--edits",
            text(&edits),
            "--out",
            text(&out),
            "--json",
        ],
        vec![
            "repack",
            text(&package),
            "--edits",
            text(&edits),
            "--out",
            text(&out),
        ],
    ] {
        let output = run(&arguments);
        assert_ne!(status(&output), 0);
        let streams = format!("{}{}", stdout(&output), stderr(&output));
        for canary in [CANARY_VALUE, CANARY_PATH, CANARY_ENTRY] {
            assert!(
                !streams.contains(canary),
                "a diagnostic carried {canary}: {streams}"
            );
        }
    }
}

#[test]
fn a_successful_report_names_no_file_and_no_value() {
    let scratch = Scratch::new("repack-quiet");
    let package = source(&scratch);
    let body = format!("\"metadata\":{{\"consignment_id\":\"{CANARY_VALUE}\"}}");
    let edits = scratch.file("edits.json", edits(&body).as_bytes());
    let out = scratch.path().join("out.krx");
    let output = repack_json(&package, &edits, &out);
    assert_eq!(status(&output), 0);
    let streams = format!("{}{}", stdout(&output), stderr(&output));
    assert!(!streams.contains(CANARY_VALUE), "{streams}");
    assert!(!streams.contains("renamed-b.txt"), "{streams}");
}

// ------------------------------------------------------------ capabilities

#[test]
fn repack_is_one_of_the_implemented_operations() {
    let response = one_object(&run(&["capabilities", "--json"]));
    let operations = response["data"]["operations"]
        .as_array()
        .expect("an operation list");
    assert!(
        operations.iter().any(|value| value == "repack"),
        "capabilities must name repack: {operations:?}"
    );
}
