//! `extract`, the one command that writes, end to end by subprocess.
//!
//! Every test here runs the built executable against a destination it owns and
//! then reads the filesystem back, because the contract this command carries
//! is about the filesystem: what was created, what was refused, and what was
//! left exactly as it was found. Asserting an exit status alone would not hold
//! any of it.
//!
//! Three tests need a symbolic link and one needs an unwritable directory.
//! Neither primitive is portable, so each is behind `cfg(unix)`. The link
//! tests have Windows counterparts in `mod junctions`, which puts a directory
//! junction — a reparse point `mklink /J` makes without elevation — in each of
//! the same three positions; the unwritable directory has a documented reason
//! for being skipped there. `docs/testing.md` records both. Everything else
//! runs on Linux, macOS and Windows alike.
mod support;

use std::path::Path;

use support::{
    ATTACHMENT_PACKAGE_DIRECTORIES, Scratch, attachment_package, attachment_package_contents,
    consistent_package, malformed_image, marker_named_entry_package, one_object, run, status,
    stderr, stdout, symlink_entry_package,
};

/// The marker an interrupted run leaves in the destination.
const MARKER: &str = ".openkrx-extract.partial";

/// A package whose third entry lives under a root directory named like the
/// marker, so the plan's directory list holds `.openkrx-extract.partial`.
///
/// The file leaf is not the marker path, so the leaf rule does not catch it;
/// the directory the run would have to create is the marker's own path.
fn marker_named_directory_package() -> Vec<u8> {
    use openkrx_core::synthetic::meta::{Document, MARKER_CONTENT, METADATA_FILE};
    use openkrx_core::synthetic::{Archive, Entry};

    let document = Document::header_only();
    Archive::of(vec![
        Entry::stored(b"mimetype", MARKER_CONTENT),
        Entry::deflated(
            format!("KRX/OCD/Metalayer/{METADATA_FILE}").as_bytes(),
            &document.bytes(),
        ),
        Entry::stored(
            b".openkrx-extract.partial/a.txt",
            b"beneath the marker name",
        ),
    ])
    .build()
}

/// Run `extract` on `image`, into `destination`, in one of the two modes.
fn extract(image: &[u8], destination: &Path, json: bool) -> std::process::Output {
    let scratch = Scratch::new("extract-input");
    let package = scratch.file("package.krx", image);
    into(&package, destination, json)
}

/// Run `extract` on an existing package file.
fn into(package: &Path, destination: &Path, json: bool) -> std::process::Output {
    let package = package.to_str().expect("a UTF-8 temporary path");
    let destination = destination.to_str().expect("a UTF-8 temporary path");
    if json {
        run(&["extract", package, "--into", destination, "--json"])
    } else {
        run(&["extract", package, "--into", destination])
    }
}

/// Every path under `root`, relative to it, with `/` separators, sorted.
fn tree(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        for entry in std::fs::read_dir(&directory).expect("read a destination directory") {
            let path = entry.expect("a directory entry").path();
            let relative = path
                .strip_prefix(root)
                .expect("a path under the destination")
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<String>>()
                .join("/");
            if std::fs::symlink_metadata(&path)
                .expect("stat a written path")
                .is_dir()
            {
                stack.push(path);
            }
            found.push(relative);
        }
    }
    found.sort();
    found
}

/// The diagnostic of a failed JSON response, with the envelope held first.
fn diagnostic(output: &std::process::Output) -> serde_json::Value {
    let value = one_object(output);
    assert_eq!(value["ok"], false);
    assert_eq!(value["verified"], false);
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["command"], "extract");
    assert!(value["data"].is_null(), "a failure carries no data");
    value["error"].clone()
}

#[test]
fn a_package_is_extracted_with_byte_identical_payloads() {
    let scratch = Scratch::new("extract-success");
    let destination = scratch.dir("out");
    let output = extract(&attachment_package(), &destination, false);
    assert_eq!(status(&output), 0, "{}", stderr(&output));
    assert_eq!(
        stderr(&output),
        "",
        "a successful run says nothing on stderr"
    );

    for (path, bytes) in attachment_package_contents() {
        let written = std::fs::read(destination.join(path)).expect("a written file");
        assert_eq!(written, bytes, "{path} was written byte for byte");
    }

    let mut expected: Vec<String> = ATTACHMENT_PACKAGE_DIRECTORIES
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    for (path, _) in attachment_package_contents() {
        expected.push(path.to_owned());
    }
    expected.sort();
    assert_eq!(tree(&destination), expected, "nothing else was created");

    let text = stdout(&output);
    assert!(text.contains("3 files written, 5 directories created"));
    assert!(text.contains("KRX/OCD/Payload/ID-1/synthetic.pdf"));
    assert!(text.contains("Nothing is verified."));
}

#[test]
fn the_json_report_names_every_file_and_the_marker_it_removed() {
    let scratch = Scratch::new("extract-json");
    let destination = scratch.dir("out");
    let output = extract(&attachment_package(), &destination, true);
    assert_eq!(status(&output), 0, "{}", stderr(&output));
    let value = one_object(&output);
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["ok"], true);
    assert_eq!(value["command"], "extract");
    assert_eq!(value["verified"], false);
    let data = &value["data"];
    assert_eq!(data["files_written"], 3);
    assert_eq!(data["directories_created"], 5);
    assert_eq!(data["bytes_written"], 1041);
    assert_eq!(data["marker_removed"], true);
    let items = data["items"].as_array().expect("items");
    let paths: Vec<&str> = items
        .iter()
        .map(|item| item["path"].as_str().expect("a path"))
        .collect();
    assert_eq!(
        paths,
        vec![
            "mimetype",
            "KRX/OCD/Metalayer/KULDEMENY_META.xml",
            "KRX/OCD/Payload/ID-1/synthetic.pdf",
        ],
        "in central-directory order, joined with / on every platform"
    );
    assert_eq!(items[0]["entry_index"], 0);
    assert_eq!(items[2]["bytes"], 19);
    assert!(
        !std::fs::exists(destination.join(MARKER)).expect("check the marker"),
        "the marker is gone when the run finished"
    );
}

#[test]
fn a_target_file_that_already_exists_refuses_the_whole_extraction() {
    let scratch = Scratch::new("extract-clobber");
    let destination = scratch.dir("out");
    std::fs::create_dir_all(destination.join("KRX/OCD/Payload/ID-1")).expect("a parent");
    let occupied = destination.join("KRX/OCD/Payload/ID-1/synthetic.pdf");
    std::fs::write(&occupied, b"a file that was already here").expect("a pre-existing file");

    let output = extract(&attachment_package(), &destination, true);
    assert_eq!(status(&output), 9);
    let error = diagnostic(&output);
    assert_eq!(error["code"], "output.exists");
    assert_eq!(error["category"], "output");
    assert_eq!(error["entry_index"], 2);
    assert_eq!(one_object(&output)["cleanup"]["removed"], 0);

    assert_eq!(
        std::fs::read(&occupied).expect("the pre-existing file"),
        b"a file that was already here",
        "the file that was already there is untouched"
    );
    assert!(
        !std::fs::exists(destination.join("mimetype")).expect("check for output"),
        "no other file was written before the refusal"
    );
    assert!(!std::fs::exists(destination.join(MARKER)).expect("check the marker"));
}

#[test]
fn an_entry_named_like_the_marker_is_refused_under_the_no_clobber_code() {
    // The marker is created before the first file, so an entry carrying its
    // name is a clash with this run's own bookkeeping. The planner has no
    // opinion about the name, so preflight must catch it — and report it as
    // the no-clobber refusal it is, not as an I/O failure part-way through.
    let scratch = Scratch::new("extract-marker-entry");
    let destination = scratch.dir("out");
    let output = extract(&marker_named_entry_package(), &destination, true);
    assert_eq!(status(&output), 9);
    let error = diagnostic(&output);
    assert_eq!(error["code"], "output.exists");
    assert_eq!(error["entry_index"], 2);
    assert_eq!(one_object(&output)["cleanup"]["removed"], 0);
    assert_eq!(
        tree(&destination),
        Vec::<String>::new(),
        "nothing was written, and no marker was left behind"
    );
}

#[test]
fn a_directory_named_like_the_marker_is_refused_under_the_no_clobber_code() {
    // The same clash one level up: nothing is planned *at* the marker path,
    // but a directory of that name would have to be created there. Preflight
    // must refuse it under the no-clobber code, before any write, rather than
    // letting `create_dir` collide with the marker and report
    // `output.not_a_directory` after the marker is already on disk.
    let scratch = Scratch::new("extract-marker-directory");
    let destination = scratch.dir("out");
    let output = extract(&marker_named_directory_package(), &destination, true);
    assert_eq!(status(&output), 9);
    let error = diagnostic(&output);
    assert_eq!(error["code"], "output.exists");
    assert_eq!(error["category"], "output");
    assert_eq!(error["entry_index"], 2, "the first entry beneath it");
    assert_eq!(one_object(&output)["cleanup"]["removed"], 0);
    assert_eq!(
        tree(&destination),
        Vec::<String>::new(),
        "nothing was written, and no marker was left behind"
    );
}

#[test]
fn a_destination_that_does_not_exist_is_refused_rather_than_created() {
    let scratch = Scratch::new("extract-absent");
    let destination = scratch.path().join("never-created");
    let output = extract(&attachment_package(), &destination, true);
    assert_eq!(status(&output), 9);
    assert_eq!(diagnostic(&output)["code"], "output.destination_missing");
    assert!(!std::fs::exists(&destination).expect("check the destination"));
    let line = stderr(&output);
    assert!(line.contains("does not exist"), "{line}");
    assert!(line.contains("(exit 9)"));
}

#[test]
fn a_destination_that_is_a_file_is_refused() {
    let scratch = Scratch::new("extract-file-destination");
    let destination = scratch.file("not-a-directory", b"file");
    let output = extract(&attachment_package(), &destination, true);
    assert_eq!(status(&output), 9);
    assert_eq!(
        diagnostic(&output)["code"],
        "output.destination_not_a_directory"
    );
}

#[test]
fn a_marker_left_by_an_interrupted_run_refuses_the_next_one() {
    let scratch = Scratch::new("extract-marker");
    let destination = scratch.dir("out");
    std::fs::write(destination.join(MARKER), b"").expect("an interrupted run's marker");
    let output = extract(&attachment_package(), &destination, true);
    assert_eq!(status(&output), 9);
    assert_eq!(diagnostic(&output)["code"], "output.partial_marker_present");
    assert!(
        !std::fs::exists(destination.join("mimetype")).expect("check for output"),
        "nothing was written"
    );
    assert!(
        std::fs::exists(destination.join(MARKER)).expect("check the marker"),
        "the marker a previous run left is not removed by this one"
    );
    assert!(stderr(&output).contains(MARKER), "the fix is named");
}

#[test]
fn a_destination_that_is_not_empty_is_accepted_and_its_contents_stay() {
    let scratch = Scratch::new("extract-not-empty");
    let destination = scratch.dir("out");
    std::fs::write(destination.join("unrelated.txt"), b"kept").expect("an unrelated file");
    let output = extract(&attachment_package(), &destination, false);
    assert_eq!(status(&output), 0, "{}", stderr(&output));
    assert_eq!(
        std::fs::read(destination.join("unrelated.txt")).expect("the unrelated file"),
        b"kept"
    );
}

#[test]
fn a_planner_refusal_keeps_its_own_category_and_names_no_path() {
    let scratch = Scratch::new("extract-planner");
    let destination = scratch.dir("out");
    let output = extract(&symlink_entry_package(), &destination, true);
    assert_eq!(
        status(&output),
        7,
        "a link is an unsupported entry, not a 9"
    );
    let error = diagnostic(&output);
    assert_eq!(error["code"], "extract.unsupported.link");
    assert_eq!(error["category"], "unsupported");
    assert_eq!(error["entry_index"], 2);
    assert_eq!(tree(&destination), Vec::<String>::new(), "nothing written");
    let line = stderr(&output);
    assert!(!line.contains("passwd"), "no entry name on stderr: {line}");
    assert!(!line.contains("ID-1"), "no entry name on stderr: {line}");
}

#[test]
fn an_unreadable_or_malformed_package_never_reaches_the_destination() {
    let scratch = Scratch::new("extract-input");
    let destination = scratch.dir("out");
    let output = extract(&malformed_image(), &destination, true);
    assert_eq!(status(&output), 6);
    assert_eq!(
        diagnostic(&output)["code"],
        "archive.malformed.eocd_missing"
    );
    assert_eq!(tree(&destination), Vec::<String>::new());

    let missing = scratch.path().join("no-such-package.krx");
    let output = into(&missing, &destination, true);
    assert_eq!(status(&output), 5);
    assert_eq!(diagnostic(&output)["code"], "input.unreadable");
    assert_eq!(tree(&destination), Vec::<String>::new());
}

#[test]
fn a_package_with_no_directories_still_reports_a_complete_run() {
    let scratch = Scratch::new("extract-flat");
    let destination = scratch.dir("out");
    // The consistent package puts its metadata under a directory, so this
    // holds the general case rather than a special one: some directories are
    // created, and the counts distinguish files from directories.
    let output = extract(&consistent_package(), &destination, true);
    assert_eq!(status(&output), 0, "{}", stderr(&output));
    let data = one_object(&output)["data"].clone();
    assert_eq!(data["files_written"], 2);
    assert_eq!(data["directories_created"], 3);
    assert_eq!(data["marker_removed"], true);
}

#[test]
fn missing_arguments_are_a_usage_error_rather_than_a_write() {
    let package = Scratch::new("extract-usage");
    let path = package.file("package.krx", &attachment_package());
    let path = path.to_str().expect("a UTF-8 temporary path");
    assert_eq!(status(&run(&["extract", path])), 2, "--into is required");
    assert_eq!(status(&run(&["extract", "--into", "."])), 2, "FILE too");
    let help = stdout(&run(&["extract", "--help"]));
    assert!(help.contains("must already exist"));
    assert!(help.contains("Nothing is ever overwritten"));
    assert!(help.contains(MARKER));
}

/// A symbolic link inside the destination, on the platforms that have one.
///
/// A symbolic link on Windows needs developer mode or an elevated process, so
/// these three are `cfg(unix)`. Their Windows counterparts are in
/// [`junctions`] below, which exercises the same three rules against a
/// directory junction — the reparse point `mklink /J` makes without any
/// privilege — and so against `FILE_ATTRIBUTE_REPARSE_POINT` in
/// `crate::extract::preflight::is_link`.
#[cfg(unix)]
mod links {
    use super::{MARKER, Scratch, attachment_package, diagnostic, extract, status, stderr, tree};

    #[test]
    fn an_ancestor_symlink_inside_the_destination_is_refused() {
        let scratch = Scratch::new("extract-ancestor-link");
        let destination = scratch.dir("out");
        let elsewhere = scratch.dir("elsewhere");
        std::os::unix::fs::symlink(&elsewhere, destination.join("KRX")).expect("a symlink");

        let output = extract(&attachment_package(), &destination, true);
        assert_eq!(status(&output), 9);
        let error = diagnostic(&output);
        assert_eq!(error["code"], "output.symlink_in_path");
        assert_eq!(error["category"], "output");
        assert!(error["entry_index"].is_number(), "the entry is named");
        assert_eq!(
            tree(&elsewhere),
            Vec::<String>::new(),
            "nothing escaped through the link"
        );
        assert!(!std::fs::exists(destination.join(MARKER)).expect("check the marker"));
    }

    #[test]
    fn a_leaf_target_that_is_a_pre_existing_symlink_is_refused() {
        let scratch = Scratch::new("extract-leaf-link");
        let destination = scratch.dir("out");
        let target = scratch.path().join("target.txt");
        std::os::unix::fs::symlink(&target, destination.join("mimetype")).expect("a symlink");

        let output = extract(&attachment_package(), &destination, true);
        assert_eq!(status(&output), 9);
        assert_eq!(diagnostic(&output)["code"], "output.exists");
        assert!(
            !std::fs::exists(&target).expect("check the link target"),
            "a dangling link is an existing path, not free space"
        );
    }

    #[test]
    fn a_destination_that_is_itself_a_symlink_is_refused() {
        let scratch = Scratch::new("extract-dest-link");
        let real = scratch.dir("real");
        let link = scratch.path().join("link");
        std::os::unix::fs::symlink(&real, &link).expect("a symlink");

        let output = extract(&attachment_package(), &link, true);
        assert_eq!(status(&output), 9);
        assert_eq!(diagnostic(&output)["code"], "output.destination_symlink");
        assert_eq!(tree(&real), Vec::<String>::new(), "nothing was written");
        assert!(stderr(&output).contains("symbolic link"));
    }
}

/// The same three rules on Windows, against a directory junction.
///
/// A junction is a reparse point, not a symbolic link, and `mklink /J` makes
/// one without developer mode or elevation. That is what lets the Windows
/// runner exercise the half of `crate::extract::preflight::is_link` that reads
/// `FILE_ATTRIBUTE_REPARSE_POINT` — the only defence this platform has against
/// output escaping the destination the caller named.
///
/// Each test returns early, with a `SKIPPED <test>:` line already printed, if
/// `mklink` is not there at all: a rule that could not be exercised must say
/// so rather than fail.
#[cfg(windows)]
mod junctions {
    use super::{MARKER, Scratch, attachment_package, diagnostic, extract, status, stderr, tree};
    use crate::support::junction;

    #[test]
    fn an_ancestor_junction_inside_the_destination_is_refused() {
        let scratch = Scratch::new("extract-ancestor-junction");
        let destination = scratch.dir("out");
        let elsewhere = scratch.dir("elsewhere");
        if !junction(&destination.join("KRX"), &elsewhere) {
            return;
        }

        let output = extract(&attachment_package(), &destination, true);
        assert_eq!(status(&output), 9);
        let error = diagnostic(&output);
        assert_eq!(error["code"], "output.symlink_in_path");
        assert_eq!(error["category"], "output");
        assert!(error["entry_index"].is_number(), "the entry is named");
        assert_eq!(
            tree(&elsewhere),
            Vec::<String>::new(),
            "nothing escaped through the junction"
        );
        assert!(!std::fs::exists(destination.join(MARKER)).expect("check the marker"));
    }

    #[test]
    fn a_leaf_target_that_is_a_pre_existing_junction_is_refused() {
        let scratch = Scratch::new("extract-leaf-junction");
        let destination = scratch.dir("out");
        let target = scratch.dir("target");
        if !junction(&destination.join("mimetype"), &target) {
            return;
        }

        let output = extract(&attachment_package(), &destination, true);
        assert_eq!(status(&output), 9);
        assert_eq!(diagnostic(&output)["code"], "output.exists");
        assert_eq!(
            tree(&target),
            Vec::<String>::new(),
            "a junction is an existing path, not free space, and its target \
was never written through"
        );
        assert!(!std::fs::exists(destination.join(MARKER)).expect("check the marker"));
    }

    #[test]
    fn a_destination_that_is_itself_a_junction_is_refused() {
        let scratch = Scratch::new("extract-dest-junction");
        let real = scratch.dir("real");
        let link = scratch.path().join("link");
        if !junction(&link, &real) {
            return;
        }

        let output = extract(&attachment_package(), &link, true);
        assert_eq!(status(&output), 9);
        assert_eq!(diagnostic(&output)["code"], "output.destination_symlink");
        assert_eq!(tree(&real), Vec::<String>::new(), "nothing was written");
        assert!(stderr(&output).contains("symbolic link"));
    }
}

/// Injected I/O failure, and the cleanup pass it triggers.
///
/// The injection makes a directory inside the destination read-only, which is
/// a Unix permission model. Windows ACL inheritance does not produce the same
/// effect from a single `set_permissions` call, and an administrator runner
/// ignores the read-only attribute on a directory entirely, so this is skipped
/// there: `docs/testing.md` records the reason. The cleanup path itself is
/// platform-independent code.
#[cfg(unix)]
mod injection {
    use std::os::unix::fs::PermissionsExt;

    use super::{
        MARKER, Scratch, attachment_package, diagnostic, extract, one_object, status, stderr,
    };

    #[test]
    fn a_failed_write_removes_this_runs_files_and_leaves_everything_else() {
        let scratch = Scratch::new("extract-readonly");
        let destination = scratch.dir("out");
        // A pre-existing file the cleanup pass must not touch.
        std::fs::write(destination.join("unrelated.txt"), b"kept").expect("an unrelated file");
        // The payload directory and its parents already exist, so the run
        // gets as far as writing files; `KRX/OCD/Metalayer` does not, so this
        // run creates one directory of its own. The payload directory is
        // read-only, so the first two files are written and the third cannot
        // be: exactly the interrupted write the cleanup policy exists for.
        let blocked = destination.join("KRX/OCD/Payload/ID-1");
        std::fs::create_dir_all(&blocked).expect("directories this run does not create");
        std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o500))
            .expect("make it read-only");
        if std::fs::write(blocked.join("probe"), b"probe").is_ok() {
            // A process that ignores the mode — root, or a filesystem without
            // permission enforcement — cannot exercise this path at all.
            std::fs::remove_file(blocked.join("probe")).expect("remove the probe");
            std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o700))
                .expect("restore the mode");
            eprintln!("skipped: this process can write into a read-only directory");
            return;
        }

        let output = extract(&attachment_package(), &destination, true);
        // Restore the mode before asserting, so a failing assertion still
        // leaves a removable directory behind for the scratch drop.
        std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o700))
            .expect("restore the mode");

        assert_eq!(status(&output), 9, "{}", stderr(&output));
        let error = diagnostic(&output);
        assert_eq!(error["code"], "output.io");
        assert_eq!(
            error["entry_index"], 2,
            "the entry that could not be written"
        );
        let cleanup = one_object(&output)["cleanup"].clone();
        assert_eq!(
            cleanup["removed"], 4,
            "the marker, the KRX/OCD/Metalayer directory this run created, and \
the two files written before the failure: {cleanup}"
        );
        assert_eq!(cleanup["left_in_place"], 0);

        assert_eq!(
            std::fs::read(destination.join("unrelated.txt")).expect("the unrelated file"),
            b"kept",
            "nothing that was already there was removed"
        );
        assert!(
            std::fs::exists(&blocked).expect("check the pre-existing directories"),
            "a directory this run did not create is never removed"
        );
        assert!(
            !std::fs::exists(destination.join("mimetype")).expect("check for output"),
            "a file this run wrote before failing is gone again"
        );
        assert!(
            !std::fs::exists(destination.join("KRX/OCD/Metalayer/KULDEMENY_META.xml"))
                .expect("check for output"),
            "and so is the second one"
        );
        assert!(
            !std::fs::exists(destination.join(MARKER)).expect("check the marker"),
            "the marker this run created is gone again"
        );
        assert!(
            !std::fs::exists(destination.join("KRX/OCD/Metalayer")).expect("check the directory"),
            "a directory this run created is removed too"
        );
        assert!(stderr(&output).contains("4 paths"), "{}", stderr(&output));
    }
}
