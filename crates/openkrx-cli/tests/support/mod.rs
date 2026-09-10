//! Subprocess helpers and the synthetic packages the command tests read.
//!
//! Every package is built in memory by `openkrx_core::synthetic`, the
//! test-only writer, and written to a temporary file the test owns. Nothing is
//! committed as a fixture and no real package is ever involved.

#![allow(dead_code)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

/// Distinguishes two scratch directories created in the same process.
///
/// A timestamp is not enough on its own: Windows' `FILETIME` has 100 ns
/// resolution, so two threads asking for the time inside the same tick get
/// the same number, and two test binaries use the same labels.
static SCRATCH_SEQUENCE: AtomicU64 = AtomicU64::new(0);

use openkrx_core::synthetic::meta::{Attachment, Document, MARKER_CONTENT, METADATA_FILE, krx};
use openkrx_core::synthetic::{Archive, Entry};

/// A canary that must never appear in a diagnostic: the path segment.
pub const CANARY_PATH: &str = "canary-directory-9f2a";
/// A canary that must never appear in a diagnostic: an entry name.
pub const CANARY_ENTRY: &str = "canary-entry-4b71.bin";
/// A canary that must never appear in a diagnostic: a metadata value.
pub const CANARY_VALUE: &str = "canary-value-c58d";

/// Run the executable with `args` and no standard input.
#[must_use]
pub fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_openkrx"))
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("run the openkrx executable")
}

/// Run the executable with `args` from inside `directory`.
///
/// The working directory matters for exactly one argument shape: a `--out`
/// that names a bare file name, whose parent is the directory the process is
/// already in.
#[must_use]
pub fn run_in(directory: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_openkrx"))
        .args(args)
        .current_dir(directory)
        .stdin(Stdio::null())
        .output()
        .expect("run the openkrx executable")
}

/// Run the executable with `bytes` on standard input.
#[must_use]
pub fn run_stdin(args: &[&str], bytes: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_openkrx"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the openkrx executable");
    let mut stdin = child.stdin.take().expect("piped standard input");
    let written = bytes.to_vec();
    let writer = std::thread::spawn(move || {
        // A refused input closes the pipe early, which is the behaviour under
        // test rather than a failure of the test.
        let _ = stdin.write_all(&written);
    });
    let output = child.wait_with_output().expect("collect the output");
    writer.join().expect("join the writer thread");
    output
}

/// The process exit status, which is always present on every supported OS.
#[must_use]
pub fn status(output: &Output) -> i32 {
    output.status.code().expect("an exit status, not a signal")
}

/// Standard output as text; every report this executable writes is UTF-8.
#[must_use]
pub fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("UTF-8 standard output")
}

/// Standard error as text.
#[must_use]
pub fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("UTF-8 standard error")
}

/// Parse standard output as exactly one JSON object.
#[must_use]
pub fn one_object(output: &Output) -> serde_json::Value {
    let text = stdout(output);
    assert_eq!(text.lines().count(), 1, "exactly one line on stdout");
    serde_json::from_str(&text).expect("one JSON object on stdout")
}

/// A directory that removes itself, holding the packages one test writes.
pub struct Scratch {
    path: PathBuf,
}

impl Scratch {
    /// Create a uniquely named directory under the platform temporary root.
    ///
    /// The name carries the process id and a per-process counter as well as a
    /// timestamp, because the labels are command names and several test
    /// binaries run in parallel: on Windows the clock's 100 ns resolution lets
    /// two of them read the same nanosecond, and two tests sharing a directory
    /// would delete each other's package when the first one finished.
    /// `create_dir` rather than `create_dir_all` makes any remaining collision
    /// a loud failure instead of a silently shared directory.
    #[must_use]
    pub fn new(label: &str) -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("a clock after 1970")
            .as_nanos();
        let process = std::process::id();
        let sequence = SCRATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("openkrx-{label}-{process}-{sequence}-{unique}"));
        std::fs::create_dir(&path).expect("create a scratch directory of this test's own");
        Self { path }
    }

    /// Write `bytes` to `name` inside the directory and return its path.
    #[must_use]
    pub fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.path.join(name);
        std::fs::write(&path, bytes).expect("write a synthetic package");
        path
    }

    /// Write `bytes` inside a nested directory named after the canary.
    #[must_use]
    pub fn canary_file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let directory = self.path.join(CANARY_PATH);
        std::fs::create_dir_all(&directory).expect("create the canary directory");
        let path = directory.join(name);
        std::fs::write(&path, bytes).expect("write a synthetic package");
        path
    }

    /// Create a subdirectory of this scratch directory and return its path.
    ///
    /// `create_dir` rather than `create_dir_all`, so a name used twice in one
    /// test is a loud failure rather than a silently shared directory.
    #[must_use]
    pub fn dir(&self, name: &str) -> PathBuf {
        let path = self.path.join(name);
        std::fs::create_dir(&path).expect("create a destination directory");
        path
    }

    /// The directory itself.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Create a directory junction at `link` pointing at `target`.
///
/// A junction is the one reparse point a Windows runner can make without any
/// privilege: `mklink /J` needs neither developer mode nor elevation, and it
/// is a `cmd` builtin, so a subprocess stands in for the reparse-point call
/// this workspace cannot make while `unsafe_code` is forbidden.
///
/// Returns `false`, after printing one `SKIPPED` line naming the test, when
/// `mklink` itself did not produce the junction. A caller must return early on
/// `false` rather than assert, so that a runner without the builtin says so
/// instead of failing a rule it never exercised. `println!` rather than
/// `eprintln!` because libtest captures both and shows what it captured on
/// failure or under `--show-output`, and `--nocapture` passes it straight
/// through; the word SKIPPED is there to be greppable in a log where a
/// skipped case would otherwise read as a pass, and
/// the test name — read from the thread libtest runs the case on — says which
/// rule went unexercised.
///
/// `cmd` wants backslashes. Every path a test builds comes from `Path::join`,
/// so its separators are already the platform's; the replacement covers only a
/// forward slash the temporary root itself might carry.
///
/// Both paths are asserted to hold none of `&^|<>"` before anything is
/// spawned. They are scratch paths this module composed, so a metacharacter
/// could only arrive from the temporary root, and `cmd` would read one as
/// syntax rather than as part of a name: a loud failure is the honest outcome
/// there, not a junction quietly made somewhere else.
#[cfg(windows)]
#[must_use]
pub fn junction(link: &Path, target: &Path) -> bool {
    /// The characters `cmd` reads as syntax rather than as part of a name.
    const METACHARACTERS: [char; 6] = ['&', '^', '|', '<', '>', '"'];

    fn backslashes(path: &Path) -> String {
        let text = path.to_string_lossy().replace('/', "\\");
        assert!(
            !text.contains(METACHARACTERS),
            "a path bound for `cmd /c mklink /J` holds one of {METACHARACTERS:?}, \
which `cmd` would read as syntax"
        );
        text
    }

    let made = Command::new("cmd")
        .args([
            "/c",
            "mklink",
            "/J",
            &backslashes(link),
            &backslashes(target),
        ])
        .stdin(Stdio::null())
        .output();
    match made {
        Ok(output) if output.status.success() && std::fs::symlink_metadata(link).is_ok() => true,
        _ => {
            // libtest names each test's thread after the test, which is how a
            // helper this far from the case can still say which one it
            // skipped. The placeholder covers only a thread with no name at
            // all, so that the line keeps its shape rather than losing a
            // field.
            let test = std::thread::current().name().unwrap_or("<test>").to_owned();
            println!("SKIPPED {test}: mklink /J unavailable");
            false
        }
    }
}

/// The timestamp every creation test writes, so that its bytes are fixed.
pub const MANIFEST_TIMESTAMP: &str = "2026-01-02T03:04:06";

/// A manifest carrying the header every creation test shares, and
/// `attachments` verbatim as the elements of its `attachments` array.
///
/// It is written as text rather than serialised from a struct, because what is
/// under test is how the executable reads a document a person typed — including
/// the keys it has to refuse.
#[must_use]
pub fn manifest(attachments: &str) -> String {
    format!(
        "{{\"schema_version\":1,\
\"timestamp\":\"{MANIFEST_TIMESTAMP}\",\
\"metadata\":{{\
\"version\":\"0.9\",\
\"source_system\":\"KER\",\
\"consignment_id\":\"SYNTHETIC-CONSIGNMENT-1\",\
\"created_at\":\"2026-01-02T03:04:06\",\
\"consignment_kind\":\"KULDEMENY\",\
\"test\":true}},\
\"attachments\":[{attachments}]}}"
    )
}

/// A package whose every check either passes or does not apply.
///
/// It declares no attachment, because a declared attachment makes check 11
/// undecided under rule M13 whatever the package looks like.
#[must_use]
pub fn consistent_package() -> Vec<u8> {
    krx(
        "KRX/OCD/",
        METADATA_FILE,
        &Document::header_only().bytes(),
        &[],
    )
}

/// The canonical layout with one resolvable attachment; M13 stays undecided.
#[must_use]
pub fn attachment_package() -> Vec<u8> {
    krx(
        "KRX/OCD/",
        METADATA_FILE,
        &Document::default().bytes(),
        &["KRX/OCD/Payload/ID-1/synthetic.pdf"],
    )
}

/// The same document under one of the other two layouts rule A19 describes.
#[must_use]
pub fn layout_package(root_prefix: &str) -> Vec<u8> {
    krx(
        root_prefix,
        METADATA_FILE,
        &Document::header_only().bytes(),
        &[],
    )
}

/// The destination path and exact bytes of every entry of
/// [`attachment_package`], in central-directory order.
///
/// The extraction tests compare what was written against these rather than
/// against a re-read of the archive, so a writer that silently transformed
/// its bytes could not pass by transforming the expectation too.
#[must_use]
pub fn attachment_package_contents() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("mimetype", MARKER_CONTENT.to_vec()),
        (
            "KRX/OCD/Metalayer/KULDEMENY_META.xml",
            Document::default().bytes(),
        ),
        (
            "KRX/OCD/Payload/ID-1/synthetic.pdf",
            b"synthetic payload 0".to_vec(),
        ),
    ]
}

/// The directories [`attachment_package`] would create, parent-first.
pub const ATTACHMENT_PACKAGE_DIRECTORIES: [&str; 5] = [
    "KRX",
    "KRX/OCD",
    "KRX/OCD/Metalayer",
    "KRX/OCD/Payload",
    "KRX/OCD/Payload/ID-1",
];

/// A package whose third entry declares itself a symbolic link.
///
/// The planner refuses the whole plan with `extract.unsupported.link`; the
/// command must surface that as an unsupported feature and write nothing.
#[must_use]
pub fn symlink_entry_package() -> Vec<u8> {
    let document = Document::header_only();
    Archive::of(vec![
        Entry::stored(b"mimetype", MARKER_CONTENT),
        Entry::deflated(
            format!("KRX/OCD/Metalayer/{METADATA_FILE}").as_bytes(),
            &document.bytes(),
        ),
        Entry::stored(b"KRX/OCD/Payload/ID-1/link", b"../../../../etc/passwd")
            .with_unix_mode(0o120_777),
    ])
    .build()
}

/// A package whose third entry is named exactly like the extraction marker.
///
/// The planner has no opinion about the name — nothing is unsafe about it —
/// so it reaches the filesystem layer, where the marker this run creates
/// occupies the same path. The no-clobber rule must refuse it before any
/// write, rather than leaving it to fail as an I/O error part-way through.
#[must_use]
pub fn marker_named_entry_package() -> Vec<u8> {
    let document = Document::header_only();
    Archive::of(vec![
        Entry::stored(b"mimetype", MARKER_CONTENT),
        Entry::deflated(
            format!("KRX/OCD/Metalayer/{METADATA_FILE}").as_bytes(),
            &document.bytes(),
        ),
        Entry::stored(b".openkrx-extract.partial", b"not this run's marker"),
    ])
    .build()
}

/// A package declaring an attachment the archive does not hold.
#[must_use]
pub fn missing_attachment_package() -> Vec<u8> {
    krx("KRX/OCD/", METADATA_FILE, &Document::default().bytes(), &[])
}

/// A package whose declared values and entry names are canaries.
#[must_use]
pub fn canary_package() -> Vec<u8> {
    let document = Document {
        consignment_id: Some(CANARY_VALUE.to_owned()),
        attachments: vec![Attachment::new(1, CANARY_ENTRY, "KRX/OCD/Payload/ID-1")],
        ..Document::default()
    };
    krx(
        "KRX/OCD/",
        METADATA_FILE,
        &document.bytes(),
        &[&format!("KRX/OCD/Payload/ID-1/{CANARY_ENTRY}")],
    )
}

/// A package carrying an entry name that is not valid UTF-8.
#[must_use]
pub fn non_utf8_name_package() -> Vec<u8> {
    let mut name = b"KRX/OCD/Payload/ID-1/".to_vec();
    name.extend_from_slice(&[0xff, 0xfe, b'.', b'b', b'i', b'n']);
    package_with_extra_entry(&name)
}

/// A package carrying a name with a C1 control and a bidirectional override.
///
/// A C0 control or `DEL` in a name is refused by the inventory itself, so the
/// characters a terminal must be protected from here are the ones that reach a
/// renderer at all.
#[must_use]
pub fn control_name_package() -> Vec<u8> {
    let mut name = "KRX/OCD/Payload/ID-1/a\u{0085}b\u{202e}c.bin"
        .as_bytes()
        .to_vec();
    name.extend_from_slice(b".bin");
    package_with_extra_entry(&name)
}

/// The canonical layout plus one extra payload entry named exactly `name`.
fn package_with_extra_entry(name: &[u8]) -> Vec<u8> {
    let document = Document::header_only();
    Archive::of(vec![
        Entry::stored(b"mimetype", MARKER_CONTENT),
        Entry::deflated(
            format!("KRX/OCD/Metalayer/{METADATA_FILE}").as_bytes(),
            &document.bytes(),
        ),
        Entry::stored(name, b"synthetic payload"),
    ])
    .build()
}

/// An archive image that is not a ZIP at all.
#[must_use]
pub fn malformed_image() -> Vec<u8> {
    b"this is not a ZIP archive".to_vec()
}

/// An archive carrying a ZIP64 extra field, which this reader refuses.
#[must_use]
pub fn zip64_image() -> Vec<u8> {
    let mut entry = Entry::stored(b"mimetype", MARKER_CONTENT);
    // Header id 0x0001 with an eight-byte uncompressed size.
    entry.central_extra = vec![0x01, 0x00, 0x08, 0x00, 0, 0, 0, 0, 0, 0, 0, 0];
    Archive::of(vec![entry]).build()
}

/// An archive declaring more entries than `Limits::DEFAULT.max_entries`.
#[must_use]
pub fn over_entry_limit_image() -> Vec<u8> {
    let entries = (0..300)
        .map(|index| Entry::stored(format!("entry-{index:04}.bin").as_bytes(), b"x"))
        .collect();
    Archive::of(entries).build()
}
