//! Write the synthetic packages the limit measurement drives the CLI with.
//!
//! `scripts/measure-limits.py` measures what the published limits cost in peak
//! resident memory and wall time. It needs, for each measured limit, one
//! package that sits exactly at the limit and one that sits one step past it,
//! and it must not build them itself: the limits, the archive layout and the
//! metadata grammar all live in this crate, and a second writer in another
//! language would drift away from them silently. This example is therefore the
//! one generator, built from the test-only writer in `openkrx_core::synthetic`
//! behind the `synthetic-writer` feature, exactly as the golden fixtures are.
//!
//! Run it from the repository root, naming a directory outside the repository:
//!
//! ```sh
//! cargo run --release -p openkrx-core --features synthetic-writer \
//!     --example limit_packages -- /tmp/openkrx-limits
//! ```
//!
//! Nothing it writes is committed. The packages are large by construction —
//! one of them decodes 128 MiB — and they are inputs to a measurement, not
//! fixtures: `tests/fixtures/README.md` keeps the committed set to the golden
//! packages alone.
//!
//! Every at-limit package is checked here, before it is written, against
//! `archive::inventory`, `profile::check` and `extract::plan` with the default
//! limits, so a package the measurement treats as "accepted at the limit" is
//! one this crate really accepts. The over-limit packages are deliberately
//! *not* checked here: the code each of them must be refused with is asserted
//! end to end, through the executable, by the measurement script.
//!
//! Alongside the packages the example writes `packages.json`, which names each
//! case, the limit it exercises, the value the limit carries, what the two
//! packages measure on that scale, and the stable code the over-limit package
//! must be refused with. The script reads that file rather than knowing any of
//! it, so adding a case here is all it takes to measure one.
//!
//! Nothing here is a conforming package: `docs/profile.md` leaves essential
//! rules unresolved, so each package is a shape to measure, not a verdict.

use std::path::Path;

use openkrx_core::extract::{self, ExtractLimits};
use openkrx_core::synthetic::meta::{Attachment, Document, MARKER_CONTENT, METADATA_FILE};
use openkrx_core::synthetic::{Archive, Entry, pseudo_random};
use openkrx_core::{Limits, MetadataLimits, archive, metadata, profile};

/// The archive-root prefix every entry but the format marker carries (A19).
const ROOT: &str = "KRX/OCD/";
/// Block size the compressible payload repeats, inside deflate's 32 KiB window
/// so that every repeat is found as a match, and large enough that the bytes a
/// match costs are negligible beside it.
const BLOCK: usize = 16384;
/// Attachments the metadata-text packages spread their text over.
const TEXT_ATTACHMENTS: usize = 200;
/// Entries the total-decoded packages spread their bytes over, chosen so that
/// no entry comes near `max_entry_decoded_bytes` at 128 MiB in total.
const TOTAL_ENTRIES: u64 = 8;

fn main() {
    let mut arguments = std::env::args().skip(1);
    let Some(directory) = arguments.next() else {
        eprintln!("usage: limit_packages <directory>");
        std::process::exit(2);
    };
    let directory = Path::new(&directory);
    std::fs::create_dir_all(directory).expect("create the package directory");
    let cases = cases();
    let mut manifest = String::from("{\n  \"cases\": [\n");
    for (index, case) in cases.iter().enumerate() {
        let at = format!("{}-at.krx", case.dimension);
        let over = format!("{}-over.krx", case.dimension);
        write(directory, &at, &case.at);
        write(directory, &over, &case.over);
        manifest.push_str(&case.json(&at, &over));
        manifest.push_str(if index + 1 == cases.len() {
            "\n"
        } else {
            ",\n"
        });
    }
    manifest.push_str("  ]\n}\n");
    let path = directory.join("packages.json");
    std::fs::write(&path, manifest.as_bytes()).expect("write the case manifest");
    println!("wrote {}", path.display());
}

/// Write one package and report its size, so a slow run shows progress.
fn write(directory: &Path, name: &str, bytes: &[u8]) {
    let path = directory.join(name);
    std::fs::write(&path, bytes).expect("write a limit package");
    println!("wrote {} ({} bytes)", path.display(), bytes.len());
}

/// One measured limit, with the package at it and the package past it.
struct Case {
    /// Short name used for the file names and the table row.
    dimension: &'static str,
    /// The field in `Limits`, `MetadataLimits` or `ExtractLimits`.
    limit: &'static str,
    /// The default that field carries.
    limit_value: u64,
    /// What the two `measured` numbers count.
    unit: &'static str,
    /// Where the at-limit package sits on that scale.
    at_measured: u64,
    /// Where the over-limit package sits on that scale.
    over_measured: u64,
    /// The stable code the over-limit package must be refused with.
    code: &'static str,
    /// Bytes of the at-limit package.
    at: Vec<u8>,
    /// Bytes of the over-limit package.
    over: Vec<u8>,
}

impl Case {
    /// The case as one object of the manifest array.
    fn json(&self, at: &str, over: &str) -> String {
        format!(
            "    {{\"dimension\": \"{}\", \"limit\": \"{}\", \"limit_value\": {}, \
\"unit\": \"{}\", \"code\": \"{}\", \
\"at\": {{\"file\": \"{at}\", \"measured\": {}, \"bytes\": {}}}, \
\"over\": {{\"file\": \"{over}\", \"measured\": {}, \"bytes\": {}}}}}",
            self.dimension,
            self.limit,
            self.limit_value,
            self.unit,
            self.code,
            self.at_measured,
            self.at.len(),
            self.over_measured,
            self.over.len(),
        )
    }
}

/// Every measured case, in table order.
fn cases() -> Vec<Case> {
    vec![
        entry_count(),
        metadata_text(),
        attachment_bytes(),
        total_decoded(),
        compression_ratio(),
        path_depth(),
    ]
}

// ------------------------------------------------------------ the packages

/// `Limits::DEFAULT.max_entries`: a full directory, and one entry more.
fn entry_count() -> Case {
    let limit = u64::from(Limits::DEFAULT.max_entries);
    let build = |entries: u64| {
        let payloads = (0..entries - 2)
            .map(|index| Payload::small(&payload_name(index), b"synthetic payload"))
            .collect::<Vec<_>>();
        assemble(&payloads)
    };
    Case {
        dimension: "entries",
        limit: "Limits::max_entries",
        limit_value: limit,
        unit: "entries",
        at_measured: limit,
        over_measured: limit + 1,
        code: "archive.over_limit.entries",
        at: checked(build(limit)),
        over: build(limit + 1),
    }
}

/// `MetadataLimits::DEFAULT.max_text_bytes`: the largest document the parser
/// accepts, and the same document one byte longer.
///
/// The boundary is found by asking the parser rather than by modelling it: the
/// padding is binary-searched for the largest value `metadata::parse` accepts,
/// so the at-limit document sits exactly on the boundary this build enforces
/// and the over-limit document sits exactly one byte past it.
fn metadata_text() -> Case {
    let limit = MetadataLimits::DEFAULT.max_text_bytes;
    let mut low = 0_u64;
    let mut high = limit;
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if metadata::parse(&text_document(middle).bytes(), &MetadataLimits::DEFAULT).is_ok() {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    let at = text_document(low);
    let over = text_document(low + 1);
    Case {
        dimension: "metadata-text",
        limit: "MetadataLimits::max_text_bytes",
        limit_value: limit,
        unit: "bytes of text",
        at_measured: limit,
        over_measured: limit + 1,
        code: "metadata.over_limit.text_bytes",
        at: checked(assemble_with(&text_payloads(), &at)),
        over: assemble_with(&text_payloads(), &over),
    }
}

/// `Limits::DEFAULT.max_entry_decoded_bytes`: one attachment at the ceiling,
/// and one byte more.
fn attachment_bytes() -> Case {
    let limit = Limits::DEFAULT.max_entry_decoded_bytes;
    let build = |bytes: u64| {
        let size = usize::try_from(bytes).expect("a package size fits in memory");
        assemble(&[Payload::stored(&payload_name(0), pseudo_random(size))])
    };
    Case {
        dimension: "attachment-bytes",
        limit: "Limits::max_entry_decoded_bytes",
        limit_value: limit,
        unit: "decoded bytes in one entry",
        at_measured: limit,
        over_measured: limit + 1,
        code: "archive.over_limit.entry_decoded_bytes",
        at: checked(build(limit)),
        over: build(limit + 1),
    }
}

/// `Limits::DEFAULT.max_total_decoded_bytes`: a package decoding exactly the
/// ceiling across every entry, and one decoding a byte more.
///
/// The payload deflates at about eight to one, because the ceiling is twice
/// `max_archive_bytes` and the image has to stay under that one to be read at
/// all — and eight is far below the ratio ceiling, so this measures the total,
/// not a ratio refusal.
fn total_decoded() -> Case {
    let limit = Limits::DEFAULT.max_total_decoded_bytes;
    let names = (0..TOTAL_ENTRIES).map(payload_name).collect::<Vec<_>>();
    let document = document_for(&names);
    let overhead = MARKER_CONTENT.len() as u64 + document.bytes().len() as u64;
    let build = |total: u64| {
        let budget = total - overhead;
        let payloads = names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let index = index as u64;
                let mut share = budget / TOTAL_ENTRIES;
                if index + 1 == TOTAL_ENTRIES {
                    share += budget % TOTAL_ENTRIES;
                }
                let share = usize::try_from(share).expect("a payload share fits in memory");
                Payload::deflated(name, compressible(share, 8))
            })
            .collect::<Vec<_>>();
        assemble_with(&payloads, &document)
    };
    Case {
        dimension: "total-decoded",
        limit: "Limits::max_total_decoded_bytes",
        limit_value: limit,
        unit: "decoded bytes in total",
        at_measured: limit,
        over_measured: limit + 1,
        code: "archive.over_limit.total_decoded_bytes",
        at: checked(build(limit)),
        over: build(limit + 1),
    }
}

/// `Limits::DEFAULT.max_compression_ratio`: the highest ratio at or under the
/// ceiling this writer can construct, and one far past it.
///
/// The reader divides the decoded volume by the whole compressed length, so
/// the entry's own header numbers give the ratio exactly and no search through
/// the reader is needed. The repeat count is swept and the largest ratio at or
/// under the ceiling is kept; the achieved ratio is reported rather than
/// assumed, because deflate decides it.
fn compression_ratio() -> Case {
    let limit = Limits::DEFAULT.max_compression_ratio;
    let ratio = |repeats: u64| ratio_payload(repeats).ratio();
    let (mut low, mut high) = (1_u64, limit * 4);
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if ratio(middle) <= limit {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    let at = ratio_payload(low);
    let over = ratio_payload(low * 3);
    assert!(
        at.ratio() <= limit,
        "the at-limit entry is over the ceiling"
    );
    assert!(
        over.ratio() > limit,
        "the over-limit entry is under the ceiling"
    );
    Case {
        dimension: "compression-ratio",
        limit: "Limits::max_compression_ratio",
        limit_value: limit,
        unit: "decoded per compressed byte",
        at_measured: at.ratio(),
        over_measured: over.ratio(),
        code: "archive.over_limit.compression_ratio",
        at: checked(assemble(&[at])),
        over: assemble(&[over]),
    }
}

/// One entry of [`BLOCK`] bytes written `repeats` times, so its ratio is about
/// `repeats` and its decoded volume is well past the 64 KiB ratio grace.
fn ratio_payload(repeats: u64) -> Payload {
    let repeats = usize::try_from(repeats).expect("a repeat count fits in memory");
    Payload::deflated(&payload_name(0), compressible(BLOCK * repeats, repeats))
}

/// `ExtractLimits::DEFAULT.max_depth`: a destination path with exactly as many
/// components as the planner allows, and one with a component more.
///
/// This is the one measured limit the reading commands do not see: the depth
/// bounds the output an extraction would produce, so only `extract` refuses it.
fn path_depth() -> Case {
    let limit = ExtractLimits::DEFAULT.max_depth;
    let build = |components: u32| {
        let root = ROOT.matches('/').count() as u32;
        let directories = (0..components - root - 1)
            .map(|level| format!("d{level:02}/"))
            .collect::<String>();
        let name = format!("{ROOT}{directories}payload.bin");
        assemble(&[Payload::small(&name, b"synthetic payload")])
    };
    Case {
        dimension: "path-depth",
        limit: "ExtractLimits::max_depth",
        limit_value: u64::from(limit),
        unit: "destination path components",
        at_measured: u64::from(limit),
        over_measured: u64::from(limit) + 1,
        code: "extract.over_limit.depth",
        at: checked(build(limit)),
        over: build(limit + 1),
    }
}

// ------------------------------------------------------------ the building

/// One payload entry: its archive name and the entry that carries it.
struct Payload {
    /// The entry name, which is also the destination path a plan derives.
    name: String,
    /// The entry as it will be written.
    entry: Entry,
}

impl Payload {
    /// A stored entry holding `data` verbatim.
    fn stored(name: &str, data: Vec<u8>) -> Self {
        Self {
            name: name.to_owned(),
            entry: Entry::stored(name.as_bytes(), &data),
        }
    }

    /// A deflated entry decoding to `data`.
    fn deflated(name: &str, data: Vec<u8>) -> Self {
        Self {
            name: name.to_owned(),
            entry: Entry::deflated(name.as_bytes(), &data),
        }
    }

    /// A small stored entry, where the content is not what is measured.
    fn small(name: &str, data: &[u8]) -> Self {
        Self::stored(name, data.to_vec())
    }

    /// Decoded bytes divided by compressed bytes, as the reader computes it.
    fn ratio(&self) -> u64 {
        u64::from(self.entry.uncompressed_size) / (self.entry.data.len() as u64).max(1)
    }
}

/// The name of the payload entry at `index`, in the documented layout.
fn payload_name(index: u64) -> String {
    format!("{ROOT}Payload/ID-{}/payload-{index}.bin", index + 1)
}

/// A document listing every one of `names` as a resolvable attachment.
fn document_for(names: &[String]) -> Document {
    let attachments = names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let (location, file_name) = name.rsplit_once('/').expect("a payload directory");
            Attachment::new(
                i64::try_from(index + 1).expect("a small attachment number"),
                file_name,
                location,
            )
        })
        .collect::<Vec<_>>();
    Document {
        declared_count: Some(attachments.len().to_string()),
        attachments,
        ..Document::default()
    }
}

/// The payload entries the metadata-text packages carry.
fn text_payloads() -> Vec<Payload> {
    (0..TEXT_ATTACHMENTS as u64)
        .map(|index| Payload::small(&payload_name(index), b"synthetic payload"))
        .collect()
}

/// A document whose padding carries `padding` further bytes of text.
///
/// The padding is one run of ASCII inside the first description, so no escape
/// changes its length and the document grows by exactly one byte per unit.
fn text_document(padding: u64) -> Document {
    let names = (0..TEXT_ATTACHMENTS as u64)
        .map(payload_name)
        .collect::<Vec<_>>();
    let mut document = document_for(&names);
    let padding = usize::try_from(padding).expect("the padding fits in memory");
    let first = document
        .attachments
        .first_mut()
        .expect("the document lists attachments");
    let mut description = first.description.take().unwrap_or_default();
    description.push_str(&"x".repeat(padding));
    first.description = Some(description);
    document
}

/// Assemble a package from `payloads`, with a document that lists them all.
fn assemble(payloads: &[Payload]) -> Vec<u8> {
    let names = payloads
        .iter()
        .map(|payload| payload.name.clone())
        .collect::<Vec<_>>();
    let document = document_for(&names);
    assemble_with(payloads, &document)
}

/// Assemble a package from `payloads` and an explicit `document`.
///
/// The order is the documented one: the format marker first and stored (A2),
/// then the metadata document, then the payloads in the order given.
fn assemble_with(payloads: &[Payload], document: &Document) -> Vec<u8> {
    let mut entries = vec![
        Entry::stored(b"mimetype", MARKER_CONTENT),
        metadata_entry(&document.bytes()),
    ];
    entries.extend(payloads.iter().map(|payload| payload.entry.clone()));
    Archive::of(entries).build()
}

/// The metadata entry, deflated unless deflate would break the ratio ceiling.
///
/// A document listing hundreds of attachments is repetitive enough to deflate
/// past `max_compression_ratio`, and a package refused for its *metadata's*
/// ratio would measure the wrong thing on every row but one. Storing it
/// verbatim costs a megabyte at most and keeps each package on the limit it is
/// built for.
fn metadata_entry(document: &[u8]) -> Entry {
    let name = format!("{ROOT}Metalayer/{METADATA_FILE}");
    let deflated = Entry::deflated(name.as_bytes(), document);
    let ratio = u64::from(deflated.uncompressed_size) / (deflated.data.len() as u64).max(1);
    if ratio <= Limits::DEFAULT.max_compression_ratio {
        deflated
    } else {
        Entry::stored(name.as_bytes(), document)
    }
}

/// Bytes of `length` that deflate at roughly `repeats` to one.
///
/// Each [`BLOCK`] of fresh pseudo-random bytes is written `repeats` times in a
/// row. The repeats are one block apart, well inside deflate's 32 KiB window,
/// so each is emitted as a match while the block itself stays incompressible:
/// the ratio is therefore about `repeats` whatever the length.
fn compressible(length: usize, repeats: usize) -> Vec<u8> {
    let blocks = length.div_ceil(BLOCK * repeats).max(1);
    let noise = pseudo_random(blocks * BLOCK);
    let mut out = Vec::with_capacity(length);
    for block in noise.chunks(BLOCK) {
        for _ in 0..repeats {
            if out.len() >= length {
                break;
            }
            let take = BLOCK.min(length - out.len());
            out.extend_from_slice(&block[..take]);
        }
    }
    out.truncate(length);
    out
}

/// Return `image` after asserting that this crate accepts it at every layer.
///
/// An at-limit package the measurement calls "accepted" has to be one the
/// reader, the structural checks and the planner all accept, so each of the
/// three is run here with the default limits and a refusal panics with the
/// error rather than being written out and measured as a success.
fn checked(image: Vec<u8>) -> Vec<u8> {
    let inventory = archive::inventory(&image, &Limits::DEFAULT)
        .expect("an at-limit package is a readable archive");
    profile::check(&inventory, &MetadataLimits::DEFAULT)
        .expect("an at-limit package passes the structural checks");
    extract::plan(&inventory, &ExtractLimits::DEFAULT)
        .expect("an at-limit package can be planned for extraction");
    image
}
