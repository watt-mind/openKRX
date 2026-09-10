//! Write the committed golden fixture packages.
//!
//! The golden output contract (`scripts/golden.py`, `tests/golden/`) needs the
//! same bytes on every machine and in every run, so its packages are the one
//! place in this repository where a synthetic archive is committed rather than
//! built inside the test that reads it. This example is what builds them, from
//! the test-only writer in `openkrx_core::synthetic`, so the provenance of
//! every committed byte is a function in this repository and nothing else.
//!
//! Run it from the repository root:
//!
//! ```sh
//! cargo run -p openkrx-core --features synthetic-writer \
//!     --example golden_fixtures -- tests/fixtures/golden
//! ```
//!
//! Every value below is fixed: no clock, no environment, no randomness, and
//! the ZIP writer stores a zero modification time and date, so re-running the
//! example over an existing directory rewrites byte-identical files.
//! `python3 scripts/golden.py verify-fixtures` asserts exactly that.
//!
//! Nothing here is a conforming package: `docs/profile.md` leaves essential
//! rules unresolved, so each fixture is a shape to observe, not a verdict.

use std::path::Path;

use openkrx_core::synthetic::meta::{Attachment, Document, METADATA_FILE, krx};
use openkrx_core::synthetic::{Archive, Entry};

/// Payload entries of the canonical fixture, and of its no-prefix twin.
const PAYLOADS: [&str; 2] = [
    "Payload/ID-1/synthetic-a.pdf",
    "Payload/ID-2/synthetic-b.pdf",
];

/// Bytes kept from the canonical image to make the truncated fixture.
///
/// The end-of-central-directory record lives at the end of a ZIP image, so
/// cutting the image in half removes it along with most of the central
/// directory: the reader then refuses the file as malformed rather than
/// reading a shorter but self-consistent archive.
const TRUNCATE_TO: usize = 2;

/// Entries of the over-limit fixture, above `Limits::DEFAULT.max_entries`.
const OVER_LIMIT_ENTRIES: u32 = 300;

fn main() {
    let mut arguments = std::env::args().skip(1);
    let Some(directory) = arguments.next() else {
        eprintln!("usage: golden_fixtures <directory>");
        std::process::exit(2);
    };
    let directory = Path::new(&directory);
    std::fs::create_dir_all(directory).expect("create the fixture directory");
    for (name, bytes) in fixtures() {
        let path = directory.join(name);
        std::fs::write(&path, &bytes).expect("write a golden fixture");
        println!("wrote {} ({} bytes)", path.display(), bytes.len());
    }
}

/// Every golden fixture, as `(file name, bytes)`, in a fixed order.
fn fixtures() -> Vec<(&'static str, Vec<u8>)> {
    let consistent = consistent();
    let mut fixtures = vec![
        ("consistent.krx", consistent.clone()),
        ("no-prefix.krx", no_prefix()),
        ("missing-attachment.krx", missing_attachment()),
        ("malformed.krx", truncated(&consistent)),
        ("over-limit.krx", over_limit()),
    ];
    fixtures.extend(manifests());
    fixtures
}

/// The inputs of the `create` golden cases.
///
/// These are not archives: they are the manifest documents and the two small
/// attachment files the creation cases point `--out` at a temporary directory
/// with. They are committed for the same reason the packages are — the golden
/// contract needs the same input bytes on every machine — and an attachment
/// path inside a manifest is resolved against the manifest's own directory,
/// which is this one.
fn manifests() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("create-attachment-a.txt", ATTACHMENT_A.to_vec()),
        ("create-attachment-b.txt", ATTACHMENT_B.to_vec()),
        ("create-manifest.json", manifest(ATTACHMENTS).into_bytes()),
        (
            "create-invalid-manifest.json",
            // `describtion` is a key the schema does not define, and the whole
            // manifest is refused rather than the value quietly dropped.
            manifest(
                "{\"path\": \"create-attachment-a.txt\", \
\"describtion\": \"a misspelled key\"}",
            )
            .into_bytes(),
        ),
        (
            "create-missing-attachment.json",
            manifest("{\"path\": \"no-such-attachment.txt\"}").into_bytes(),
        ),
    ]
}

/// The bytes of the first golden attachment.
const ATTACHMENT_A: &[u8] = b"synthetic attachment a\n";
/// The bytes of the second golden attachment.
const ATTACHMENT_B: &[u8] = b"synthetic attachment b\n";
/// The `attachments` array of the manifest that is meant to succeed.
const ATTACHMENTS: &str = "{\"path\": \"create-attachment-a.txt\", \
\"description\": \"the first synthetic attachment\"},\n    \
{\"path\": \"create-attachment-b.txt\", \"file_name\": \"renamed-b.txt\", \
\"description\": \"the second synthetic attachment\"}";

/// One manifest carrying `attachments`, with every other value fixed.
///
/// The timestamp is written out rather than read from a clock, here as
/// everywhere else: `create` has no clock, and the golden bytes must be the
/// same in every run.
fn manifest(attachments: &str) -> String {
    format!(
        "{{\n  \"schema_version\": 1,\n  \"timestamp\": \"2026-01-02T03:04:06\",\n  \
\"metadata\": {{\n    \"version\": \"0.9\",\n    \"source_system\": \"KER\",\n    \
\"consignment_id\": \"SYNTHETIC-CONSIGNMENT-1\",\n    \
\"created_at\": \"2026-01-02T03:04:06\",\n    \
\"consignment_kind\": \"KULDEMENY\",\n    \"test\": true\n  }},\n  \
\"attachments\": [\n    {attachments}\n  ]\n}}\n"
    )
}

/// A document declaring `count` attachments under `location_prefix`.
fn document(count: usize, location_prefix: &str) -> Document {
    let attachments = PAYLOADS
        .iter()
        .take(count)
        .enumerate()
        .map(|(index, payload)| {
            let (location, file_name) = payload.rsplit_once('/').expect("a payload directory");
            Attachment::new(
                i64::try_from(index + 1).expect("a small attachment number"),
                file_name,
                &format!("{location_prefix}{location}"),
            )
        })
        .collect::<Vec<_>>();
    Document {
        declared_count: Some(attachments.len().to_string()),
        attachments,
        ..Document::default()
    }
}

/// The canonical layout, with two declared attachments that both resolve.
fn consistent() -> Vec<u8> {
    let payloads = PAYLOADS
        .iter()
        .map(|payload| format!("KRX/OCD/{payload}"))
        .collect::<Vec<_>>();
    let names = payloads.iter().map(String::as_str).collect::<Vec<_>>();
    krx(
        "KRX/OCD/",
        METADATA_FILE,
        &document(PAYLOADS.len(), "KRX/OCD/").bytes(),
        &names,
    )
}

/// The same package with `Metalayer/` at the archive root instead.
///
/// Rule A19 describes three observed layouts and no primary source settles
/// which one a package must use, so this one leaves that check undecided.
fn no_prefix() -> Vec<u8> {
    let names = PAYLOADS.to_vec();
    krx(
        "",
        METADATA_FILE,
        &document(PAYLOADS.len(), "").bytes(),
        &names,
    )
}

/// The canonical layout declaring one attachment the archive does not hold.
fn missing_attachment() -> Vec<u8> {
    krx(
        "KRX/OCD/",
        METADATA_FILE,
        &document(1, "KRX/OCD/").bytes(),
        &[],
    )
}

/// The canonical image cut in half, so its end record is gone.
fn truncated(image: &[u8]) -> Vec<u8> {
    image[..image.len() / TRUNCATE_TO].to_vec()
}

/// An archive declaring more entries than `Limits::DEFAULT.max_entries`.
fn over_limit() -> Vec<u8> {
    let entries = (0..OVER_LIMIT_ENTRIES)
        .map(|index| Entry::stored(format!("entry-{index:04}.bin").as_bytes(), b"x"))
        .collect();
    Archive::of(entries).build()
}
