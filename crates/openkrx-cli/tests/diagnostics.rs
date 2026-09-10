//! What a failed run *tells the caller*, beyond its code and its status.
//!
//! `failures.rs` and `extract.rs` hold the stable code and the exit status of
//! every refusal. This file holds the other half of the diagnostic contract:
//! the one content-free sentence `exit::advice` gives each `output.*` code
//! instead of the single sentence its category would otherwise share, and the
//! numbers a limit failure carries whichever command produced it.
//!
//! The advice sentences were reachable by mutation without a test noticing
//! (see docs/testing.md, "Mutation testing"): dropping an arm falls back to
//! the category's own sentence, which still reads plausibly while telling the
//! caller the wrong thing to do.
//!
//! Every assertion below is on a sentence that names no path, no entry name
//! and no declared value, which is the rule `privacy.rs` holds in general.
mod support;

use std::path::Path;

use support::{Scratch, attachment_package, one_object, run, status, stderr};

/// Run `extract` on a package written to a temporary file.
fn extract_into(destination: &Path) -> std::process::Output {
    let scratch = Scratch::new("advice-input");
    let package = scratch.file("package.krx", &attachment_package());
    run(&[
        "extract",
        package.to_str().expect("a UTF-8 temporary path"),
        "--into",
        destination.to_str().expect("a UTF-8 temporary path"),
    ])
}

/// What a failed run writes on standard error.
///
/// A failed `extract` writes its diagnostic and, because the destination was
/// touched, one cleanup line; nothing else may appear.
fn line(output: &std::process::Output) -> String {
    let text = stderr(output);
    assert!(
        (1..=2).contains(&text.lines().count()),
        "a diagnostic and at most one cleanup line:\n{text}"
    );
    text.trim_end().to_owned()
}

#[test]
fn a_destination_that_is_not_a_directory_is_told_what_the_argument_names() {
    let scratch = Scratch::new("advice-file-destination");
    let destination = scratch.file("not-a-directory", b"file");
    let output = extract_into(&destination);
    assert_eq!(status(&output), 9);
    let line = line(&output);
    assert!(
        line.contains(
            "the --into argument, or the parent of the --out argument, names something that is \
not a directory"
        ),
        "{line}"
    );
    assert!(
        !line.contains("does not exist"),
        "the sentence is this code's own, not another output code's: {line}"
    );
}

#[test]
fn an_existing_ancestor_that_is_a_file_is_told_what_is_in_the_way() {
    let scratch = Scratch::new("advice-ancestor-file");
    let destination = scratch.dir("out");
    // The package writes into `KRX/OCD/...`; a plain file at `KRX` is a path
    // the run needs as a directory and cannot use as one.
    std::fs::write(destination.join("KRX"), b"in the way").expect("an occupying file");
    let output = extract_into(&destination);
    assert_eq!(status(&output), 9);
    let line = line(&output);
    assert!(line.contains("output.not_a_directory"), "{line}");
    assert!(
        line.contains(
            "a path this package needs as a directory is something else in the \
destination already"
        ),
        "{line}"
    );
    assert!(
        !destination.join("mimetype").exists(),
        "nothing was written before the refusal"
    );
    // The second line is the cleanup fact, and a refusal reached before any
    // write must state that the destination is untouched rather than report
    // a count of nothing.
    assert!(
        line.contains("openkrx: nothing had been written, so the destination is as it was found"),
        "{line}"
    );
}

#[test]
fn a_limit_failure_carries_its_numbers_whichever_command_reported_it() {
    // `failures.rs` holds this refusal for `list`. Every command reads the
    // inventory before it does anything else, so `validate-structure` must
    // report the same code with the same numbers rather than reducing the
    // failure to a bare code on its way through the structural command.
    let scratch = Scratch::new("advice-limit");
    let package = scratch.file("package.krx", &support::over_entry_limit_image());
    let output = run(&[
        "validate-structure",
        package.to_str().expect("a UTF-8 temporary path"),
        "--json",
    ]);
    assert_eq!(status(&output), 8);
    let value = one_object(&output);
    assert_eq!(value["ok"], false);
    let error = &value["error"];
    assert_eq!(error["code"], "archive.over_limit.entries");
    assert_eq!(error["category"], "limit");
    assert_eq!(error["limit"], 256, "the limit that was exceeded");
    assert_eq!(error["observed"], 300, "and what was observed instead");
}

/// The two conditions that need a Unix permission or link primitive.
///
/// Windows cannot create a symbolic link without developer mode or elevation,
/// and its ACL model does not make a directory unwritable through one
/// `set_permissions` call; `docs/testing.md` records the same reason for the
/// tests in `extract.rs` that are gated this way.
#[cfg(unix)]
mod unix_only {
    use std::os::unix::fs::PermissionsExt;

    use super::{Scratch, extract_into, line, status};

    #[test]
    fn an_ancestor_symlink_is_told_why_a_link_is_refused() {
        let scratch = Scratch::new("advice-ancestor-link");
        let destination = scratch.dir("out");
        let elsewhere = scratch.dir("elsewhere");
        std::os::unix::fs::symlink(&elsewhere, destination.join("KRX")).expect("a symlink");

        let output = extract_into(&destination);
        assert_eq!(status(&output), 9);
        let line = line(&output);
        assert!(line.contains("output.symlink_in_path"), "{line}");
        assert!(
            line.contains(
                "a directory this package would write through is a symbolic link \
or a reparse point, which could place output outside the destination"
            ),
            "{line}"
        );
    }

    #[test]
    fn a_write_that_fails_is_told_to_check_the_destination() {
        let scratch = Scratch::new("advice-readonly");
        let destination = scratch.dir("out");
        let mode = std::fs::metadata(&destination)
            .expect("stat the destination")
            .permissions()
            .mode();
        // Readable and searchable, not writable: the marker this run must
        // create cannot be created, which is an I/O failure rather than a
        // no-clobber refusal.
        std::fs::set_permissions(&destination, std::fs::Permissions::from_mode(0o500))
            .expect("make the destination read-only");

        // Whether the injection took is decided by a probe *before* the run,
        // the way `extract.rs`'s injection test does it, and never by the
        // command's own exit status: reading a status of zero as "this
        // process must be root" would turn a genuine regression in the write
        // path into a silent skip. `println!` rather than `eprintln!` because
        // libtest shows captured standard output under `--nocapture` and
        // `--show-output`, and the word SKIPPED is there to be greppable in
        // a log where a skipped case would otherwise read as a pass.
        if std::fs::write(destination.join("probe"), b"probe").is_ok() {
            std::fs::remove_file(destination.join("probe")).expect("remove the probe");
            std::fs::set_permissions(&destination, std::fs::Permissions::from_mode(mode))
                .expect("restore the destination");
            println!(
                "SKIPPED a_write_that_fails_is_told_to_check_the_destination: \
this process can write into a read-only directory, so the failure this test \
needs cannot be injected"
            );
            return;
        }

        let output = extract_into(&destination);

        // Restored before any assertion, so a failure still leaves a
        // removable scratch directory behind.
        std::fs::set_permissions(&destination, std::fs::Permissions::from_mode(mode))
            .expect("restore the destination");

        assert_eq!(status(&output), 9);
        let line = line(&output);
        assert!(line.contains("output.io"), "{line}");
        assert!(
            line.contains(
                "a create, write or remove failed: check that the destination is \
writable and has free space"
            ),
            "{line}"
        );
    }
}
