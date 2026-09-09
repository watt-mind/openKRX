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
    vec![
        ("consistent.krx", consistent.clone()),
        ("no-prefix.krx", no_prefix()),
        ("missing-attachment.krx", missing_attachment()),
        ("malformed.krx", truncated(&consistent)),
        ("over-limit.krx", over_limit()),
    ]
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
