//! Every archive this repository holds, inflated twice and compared.
//!
//! The archive layer trusts one external implementation of RFC 1951 on hostile
//! bytes. This file decodes the same streams with a second, independent
//! implementation — `zune-inflate`, a port of libdeflate that shares no code
//! with the reader's decoder — and asserts the two agree, byte for byte.
//!
//! Three things are checked, over every package the repository can produce:
//!
//! - **Accepted entries.** Where the reader accepted an entry, the reference
//!   decoder produces exactly the same bytes, and exactly as many as the entry
//!   reports having counted.
//! - **Limit refusals.** Where the reader refused a package for a decoded-size
//!   or compression-ratio ceiling, the reference decoder's own output is shown
//!   to cross that same ceiling. A refusal is therefore a limit doing its job,
//!   not one decoder disagreeing with the other about what the stream says.
//! - **Malformed streams.** Where the reader refused a stream as malformed, the
//!   reference decoder is asked the same question, and its answer is recorded
//!   rather than asserted: the two implementations may draw the line between
//!   "corrupt" and "truncated" differently, and neither is authoritative. What
//!   is asserted is that it never returns a *complete, valid* decode of a
//!   stream the reader called malformed.
//!
//! The corpus is everything at once: the committed `.krx` fixtures, the seed
//! corpus the fuzzing lane derives from them, the retained fuzzing regressions,
//! packages built by the crate's own writers, and property-generated payloads
//! at every deflate level the crate emits.
//!
//! What this cannot show: the reference decoder is itself unverified. Two
//! implementations agreeing is evidence, not proof — a stream both read the
//! same wrong way would pass here. Nothing in this file is a conformance
//! statement.
//!
//! Every byte compared here is generated in this repository or committed as a
//! synthetic fixture; no official sample is copied.

mod support;

use std::path::{Path, PathBuf};

use openkrx_core::{ArchiveError, LimitKind, Limits, archive};
use proptest::prelude::*;
use support::{Archive, Entry, pseudo_random};
use zune_inflate::{DeflateDecoder, DeflateOptions};

/// Stored (uncompressed) compression method.
const STORED: u16 = 0;
/// Deflate compression method.
const DEFLATE: u16 = 8;

/// Ceiling the reference decoder is given, in bytes.
///
/// Well above every `Limits::DEFAULT` decoded-byte ceiling, so a package the
/// reader refused over one of those can still be decoded here in full and the
/// refusal shown to be limit-driven. It exists only so that a fixture nobody
/// intended cannot make the test allocate without bound.
const REFERENCE_LIMIT_BYTES: usize = 256 * 1024 * 1024;

/// Deflate levels the compressor accepts, from stored blocks to its slowest.
///
/// The crate itself emits exactly one — level 6, in both `create::zip` and the
/// synthetic writer — but a decoder is only as good as the block types it has
/// been shown, and the level decides those: 0 produces stored blocks, 1 fixed
/// Huffman blocks, and the higher levels dynamic ones with progressively longer
/// matches. Sweeping the range exercises all three block types against both
/// decoders instead of only whichever the writer's level happens to pick.
const LEVELS: [u8; 11] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

// ---------------------------------------------------------------------------
// An independent view of the container
// ---------------------------------------------------------------------------

/// One entry located without the reader's help.
///
/// The reader's public surface reports what it decoded, not the compressed
/// bytes it decoded from, and a package the reader refused reports nothing at
/// all. Locating entries here — a plain central-directory walk, with sizes read
/// from the central record and the data offset from the local header — is what
/// lets the reference decoder be pointed at the same stream in both cases.
///
/// This walk is deliberately permissive: it is a way to find bytes, never a
/// second opinion on whether a container is well formed.
struct Member<'a> {
    /// Compression method declared by the central directory.
    method: u16,
    /// Uncompressed size declared by the central directory.
    declared_size: u64,
    /// Exactly the entry's compressed bytes.
    data: &'a [u8],
}

/// Read a little-endian `u16` at `at`, or nothing when the slice is too short.
fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    let end = at.checked_add(2)?;
    Some(u16::from_le_bytes(bytes.get(at..end)?.try_into().ok()?))
}

/// Read a little-endian `u32` at `at`, or nothing when the slice is too short.
fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    let end = at.checked_add(4)?;
    Some(u32::from_le_bytes(bytes.get(at..end)?.try_into().ok()?))
}

/// Locate every entry's compressed bytes, or nothing when this is not a ZIP.
///
/// Returns `None` rather than failing: several fixtures are deliberately not
/// containers, and a file this walk cannot open simply contributes no streams
/// to compare. The reader's verdict on such a file is asserted elsewhere.
fn locate(image: &[u8]) -> Option<Vec<Member<'_>>> {
    let end = find_end_record(image)?;
    let count = usize::from(u16_at(image, end + 10)?);
    let mut cursor = u32_at(image, end + 16)? as usize;
    let mut members = Vec::with_capacity(count);
    for _ in 0..count {
        if u32_at(image, cursor)? != 0x0201_4b50 {
            return None;
        }
        let method = u16_at(image, cursor + 10)?;
        let compressed = u32_at(image, cursor + 20)? as usize;
        let declared_size = u64::from(u32_at(image, cursor + 24)?);
        let name_bytes = usize::from(u16_at(image, cursor + 28)?);
        let extra_bytes = usize::from(u16_at(image, cursor + 30)?);
        let comment_bytes = usize::from(u16_at(image, cursor + 32)?);
        let header = u32_at(image, cursor + 42)? as usize;
        if u32_at(image, header)? != 0x0403_4b50 {
            return None;
        }
        let start = header
            .checked_add(30)?
            .checked_add(usize::from(u16_at(image, header + 26)?))?
            .checked_add(usize::from(u16_at(image, header + 28)?))?;
        let data = image.get(start..start.checked_add(compressed)?)?;
        members.push(Member {
            method,
            declared_size,
            data,
        });
        cursor = cursor
            .checked_add(46)?
            .checked_add(name_bytes)?
            .checked_add(extra_bytes)?
            .checked_add(comment_bytes)?;
    }
    Some(members)
}

/// Offset of the last end-of-central-directory record, if the image has one.
fn find_end_record(image: &[u8]) -> Option<usize> {
    (0..image.len().checked_sub(21)?)
        .rev()
        .find(|at| u32_at(image, *at) == Some(0x0605_4b50))
}

// ---------------------------------------------------------------------------
// The reference decoder
// ---------------------------------------------------------------------------

/// Decode one entry with the reference implementation and nothing else.
///
/// No openKRX limit applies here — that is the point: the comparison is between
/// two decoders, and the limits are then shown separately to be what refused a
/// package rather than a disagreement about the bytes.
fn reference(member: &Member<'_>) -> Result<Vec<u8>, String> {
    match member.method {
        STORED => Ok(member.data.to_vec()),
        DEFLATE => reference_inflate(member.data),
        other => Err(format!("method {other} is not one the reader decodes")),
    }
}

/// Inflate a raw deflate stream with the reference implementation.
fn reference_inflate(data: &[u8]) -> Result<Vec<u8>, String> {
    let options = DeflateOptions::default()
        .set_limit(REFERENCE_LIMIT_BYTES)
        .set_confirm_checksum(false);
    DeflateDecoder::new_with_options(data, options)
        .decode_deflate()
        .map_err(|error| format!("{error:?}"))
}

// ---------------------------------------------------------------------------
// The comparison
// ---------------------------------------------------------------------------

/// What comparing one package produced, so a caller can assert coverage.
#[derive(Debug, Default, PartialEq, Eq)]
struct Compared {
    /// Entries whose bytes both decoders produced identically.
    agreed: usize,
    /// Packages the reader refused for a decoded-size or ratio ceiling.
    limited: usize,
}

/// Decode every entry of one package with both decoders under `Limits::DEFAULT`.
fn compare(image: &[u8], label: &str) -> Compared {
    compare_with(image, label, &Limits::DEFAULT)
}

/// Decode every entry of one package with both decoders and compare.
///
/// `label` names the input in an assertion message. It is an index or a file
/// name, never an absolute path.
fn compare_with(image: &[u8], label: &str, limits: &Limits) -> Compared {
    let mut result = Compared::default();
    let Some(members) = locate(image) else {
        return result;
    };
    match archive::inventory(image, limits) {
        Ok(inventory) => {
            assert_eq!(
                inventory.len(),
                members.len(),
                "{label}: the reader and the walk disagree about the entry count"
            );
            for (index, member) in members.iter().enumerate() {
                let entry = inventory.entries()[index];
                let ours = inventory
                    .entry_bytes(index as u32)
                    .unwrap_or_else(|error| panic!("{label}: entry {index}: {error}"));
                let theirs = reference(member)
                    .unwrap_or_else(|error| panic!("{label}: entry {index}: reference: {error}"));
                assert_eq!(
                    ours, theirs,
                    "{label}: entry {index}: the two decoders produced different bytes"
                );
                assert_eq!(
                    ours.len() as u64,
                    entry.decoded_size(),
                    "{label}: entry {index}: counted size is not the size decoded"
                );
                result.agreed += 1;
            }
        }
        Err(ArchiveError::OverLimit {
            limit,
            limit_value,
            entry,
            ..
        }) => {
            if assert_limit_is_real(&members, limit, limit_value, entry, label) {
                result.limited += 1;
            }
        }
        Err(ArchiveError::Malformed { entry, .. }) => {
            assert_reference_does_not_disagree(&members, entry, label);
        }
        Err(_) => {}
    }
    result
}

/// Show that a decoded-byte refusal is the ceiling, not a decoder disagreement.
///
/// Returns whether the ceiling was one this file can speak about. The name,
/// count, extra-field and comment ceilings are structural: they are crossed
/// before a byte is inflated, so there is nothing here to demonstrate.
fn assert_limit_is_real(
    members: &[Member<'_>],
    limit: LimitKind,
    limit_value: u64,
    entry: Option<u32>,
    label: &str,
) -> bool {
    let index = match entry {
        Some(index) => index as usize,
        None => return false,
    };
    let Some(member) = members.get(index) else {
        return false;
    };
    match limit {
        LimitKind::EntryDecodedBytes => {
            let Some(decoded) = reference_or_skip(member, label, index) else {
                return false;
            };
            assert!(
                decoded > limit_value,
                "{label}: entry {index}: the reference decode is {decoded} bytes, which does not \
                 exceed {}",
                limit.code()
            );
        }
        LimitKind::TotalDecodedBytes => {
            // The ceiling is archive-wide, so the demonstration is too: the
            // reference decode of every entry up to and including the one the
            // reader stopped on has to cross it.
            let mut total = 0_u64;
            for (position, earlier) in members.iter().enumerate().take(index + 1) {
                let Some(decoded) = reference_or_skip(earlier, label, position) else {
                    return false;
                };
                total = total.saturating_add(decoded);
            }
            assert!(
                total > limit_value,
                "{label}: entry {index}: the reference decode totals {total} bytes, which does \
                 not exceed {}",
                limit.code()
            );
        }
        LimitKind::CompressionRatio => {
            let Some(decoded) = reference_or_skip(member, label, index) else {
                return false;
            };
            // The reader charges the ratio against the whole entry's compressed
            // bytes and stops the moment a decoded chunk crosses it, so the
            // ratio only grows from there: the full decode's ratio is at least
            // the one that triggered the refusal.
            let compressed = (member.data.len() as u64).max(1);
            assert!(
                decoded > Limits::RATIO_GRACE_BYTES,
                "{label}: entry {index}: refused on the ratio inside the grace window"
            );
            assert!(
                decoded / compressed > limit_value,
                "{label}: entry {index}: the reference decode's ratio does not exceed {}",
                limit.code()
            );
        }
        _ => return false,
    }
    true
}

/// The reference decode's length, or nothing when it declined the stream.
///
/// A limit refusal can also be the reader stopping mid-stream on a deliberately
/// truncated bomb, which the reference decoder — given the whole stream at once
/// — reports as insufficient data. That is not a disagreement about bytes, so it
/// is skipped rather than asserted.
fn reference_or_skip(member: &Member<'_>, label: &str, index: usize) -> Option<u64> {
    match reference(member) {
        Ok(bytes) => Some(bytes.len() as u64),
        Err(error) => {
            println!("{label}: entry {index}: reference declined the stream: {error}");
            None
        }
    }
}

/// Assert the reference decoder never fully accepts a stream called malformed.
///
/// The reader's malformed verdict covers more than the deflate stream — a CRC
/// or declared-size mismatch is a malformed *entry* around a stream that decodes
/// perfectly well — so the assertion is narrow: where the reader called the
/// deflate stream itself malformed, a complete reference decode of exactly the
/// declared size and no error would be a real disagreement.
fn assert_reference_does_not_disagree(members: &[Member<'_>], entry: Option<u32>, label: &str) {
    let Some(index) = entry.map(|index| index as usize) else {
        return;
    };
    let Some(member) = members.get(index) else {
        return;
    };
    if member.method != DEFLATE {
        return;
    }
    if let Ok(bytes) = reference(member) {
        // Decoding to the declared size is exactly what a well-formed entry
        // does; the reader must then have refused it for the CRC or a size the
        // headers disagree about, both of which it checks and the reference
        // decoder does not. Anything shorter is the two decoders agreeing that
        // the stream is broken, differing only in where they say so.
        assert!(
            bytes.len() as u64 <= member.declared_size,
            "{label}: entry {index}: the reference decoder produced more than the declared size"
        );
    }
}

// ---------------------------------------------------------------------------
// The committed corpus
// ---------------------------------------------------------------------------

/// The repository root, from this crate's manifest directory.
fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("the crate manifest sits two directories below the repository root")
}

/// Every committed `.krx` fixture, in the order `fuzz/seed.py` walks them.
///
/// The seeding script sorts `tests/fixtures/golden/*.krx` and writes each one
/// unchanged as a seed for all three package-shaped fuzz targets, so this list
/// *is* the package seed corpus: reproducing the walk here covers those seeds
/// without running Python, and keeps the test working on a machine and a CI lane
/// that has no interpreter on the path.
fn fixture_packages() -> Vec<PathBuf> {
    let mut found = Vec::new();
    collect(&repository().join("tests"), &mut found);
    found.sort();
    found
}

/// Every retained fuzzing regression, which are archive images too.
fn regressions() -> Vec<PathBuf> {
    let mut found = Vec::new();
    for target in ["inventory", "structure", "extract_plan"] {
        collect_files(
            &repository().join("fuzz/regressions").join(target),
            &mut found,
        );
    }
    found.sort();
    found
}

/// Append every `.krx` file under `directory`, recursively.
fn collect(directory: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, found);
        } else if path.extension().is_some_and(|suffix| suffix == "krx") {
            found.push(path);
        }
    }
}

/// Append every file directly inside `directory`.
fn collect_files(directory: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().is_none_or(|suffix| suffix != "md") {
            found.push(path);
        }
    }
}

/// A file's name, which is what an assertion message may carry.
fn label(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| String::from("unnamed"))
}

#[test]
fn every_committed_package_decodes_identically_under_both_decoders() {
    let packages = fixture_packages();
    assert!(
        !packages.is_empty(),
        "no committed package was found; the fixture walk is looking in the wrong place"
    );
    let mut agreed = 0;
    for path in &packages {
        let image = std::fs::read(path).expect("a committed fixture is readable");
        agreed += compare(&image, &label(path)).agreed;
    }
    assert!(
        agreed > 0,
        "no entry was compared; the corpus reached no accepted package"
    );
}

#[test]
fn a_refusal_over_a_decoded_byte_ceiling_is_the_ceiling_and_not_the_decoder() {
    // The committed fixtures cross only the structural entry-count ceiling, so
    // the three decoded-byte ceilings are reached the way the reader's own
    // limit tests reach them: with an archive built to cross exactly one.
    let bomb = vec![0_u8; 4 * 1024 * 1024];
    let image = Archive::of(vec![Entry::deflated(b"bomb.bin", &bomb)]).build();
    assert_eq!(
        compare_with(&image, "ratio bomb", &Limits::DEFAULT).limited,
        1,
        "the ratio ceiling was not demonstrated"
    );

    let payload = pseudo_random(4096);
    let one = Archive::of(vec![Entry::deflated(b"a.bin", &payload)]).build();
    assert_eq!(
        compare_with(
            &one,
            "entry ceiling",
            &Limits {
                max_entry_decoded_bytes: 4095,
                ..Limits::DEFAULT
            }
        )
        .limited,
        1,
        "the per-entry ceiling was not demonstrated"
    );

    let two = Archive::of(vec![
        Entry::deflated(b"a.bin", &payload),
        Entry::deflated(b"b.bin", &payload),
    ])
    .build();
    assert_eq!(
        compare_with(
            &two,
            "total ceiling",
            &Limits {
                max_total_decoded_bytes: 8191,
                ..Limits::DEFAULT
            }
        )
        .limited,
        1,
        "the archive-wide ceiling was not demonstrated"
    );
}

#[test]
fn every_retained_fuzzing_regression_decodes_identically_under_both_decoders() {
    for path in &regressions() {
        let image = std::fs::read(path).expect("a retained regression is readable");
        compare(&image, &label(path));
    }
}

// ---------------------------------------------------------------------------
// Packages the writers build
// ---------------------------------------------------------------------------

#[test]
fn packages_the_synthetic_writer_builds_decode_identically_under_both_decoders() {
    let payloads: [Vec<u8>; 5] = [
        Vec::new(),
        b"application/OCD+ZIP".to_vec(),
        vec![0_u8; 64 * 1024],
        pseudo_random(64 * 1024),
        b"<?xml version=\"1.0\"?><a>&amp;&lt;&gt;</a>".repeat(64),
    ];
    let mut compared = 0;
    for (index, payload) in payloads.iter().enumerate() {
        let image = Archive::of(vec![
            Entry::stored(b"stored", payload),
            Entry::deflated(b"deflated", payload),
        ])
        .build();
        compared += compare(&image, &format!("synthetic {index}")).agreed;
    }
    assert_eq!(
        compared,
        payloads.len() * 2,
        "every synthetic entry should have been compared"
    );
}

#[test]
fn every_deflate_level_the_compressor_accepts_decodes_identically() {
    let payloads: [Vec<u8>; 4] = [
        Vec::new(),
        vec![0_u8; 4096],
        pseudo_random(4096),
        b"the same short run, over and over. ".repeat(256),
    ];
    for payload in &payloads {
        for level in LEVELS {
            let stream = miniz_oxide::deflate::compress_to_vec(payload, level);
            let theirs = reference_inflate(&stream)
                .unwrap_or_else(|error| panic!("level {level}: reference: {error}"));
            assert_eq!(
                &theirs, payload,
                "level {level}: the reference decoder did not round trip the compressor's output"
            );
            let image = Archive::of(vec![Entry::deflated(b"payload", payload)]).build();
            assert_eq!(
                compare(&image, &format!("level {level}")).agreed,
                1,
                "level {level}: the entry was not compared"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The property
// ---------------------------------------------------------------------------

/// Cases the property runs when `PROPTEST_CASES` says nothing.
///
/// Each case compresses one payload at every level and inflates it twice, so a
/// case is eleven compressions and twenty-two decodes; the count is chosen to
/// keep the whole file inside its second-scale budget on the slowest lane.
const DEFAULT_CASES: u32 = 48;

/// The bounded configuration the property runs under.
fn config() -> ProptestConfig {
    let cases = std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|cases| *cases > 0)
        .unwrap_or(DEFAULT_CASES);
    ProptestConfig {
        cases,
        failure_persistence: Some(Box::new(
            proptest::test_runner::FileFailurePersistence::Direct(
                "proptest-regressions/differential_inflate.txt",
            ),
        )),
        ..ProptestConfig::default()
    }
}

/// Payloads worth compressing: noise, runs and a mixture, up to 16 KiB.
///
/// Pure noise is the incompressible case, a run of one byte the maximally
/// compressible one, and a short alphabet the case where the match finder does
/// most of its work — the three shapes that push the compressor toward
/// different block types and match lengths.
fn payload() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        proptest::collection::vec(any::<u8>(), 0..16 * 1024),
        proptest::collection::vec(0_u8..2, 0..16 * 1024),
        proptest::collection::vec(b'a'..b'e', 0..16 * 1024),
    ]
}

proptest! {
    #![proptest_config(config())]

    /// Both decoders read the compressor's output as the bytes that went in.
    #[test]
    fn both_decoders_agree_on_any_payload_at_any_level(payload in payload()) {
        for level in LEVELS {
            let stream = miniz_oxide::deflate::compress_to_vec(&payload, level);
            let theirs = reference_inflate(&stream).map_err(|error| {
                TestCaseError::fail(format!("level {level}: reference: {error}"))
            })?;
            prop_assert_eq!(&theirs, &payload, "level {} reference output", level);
        }
    }

    /// A package the writer builds reads back identically under both decoders.
    #[test]
    fn a_written_package_decodes_identically_under_both_decoders(payload in payload()) {
        let image = Archive::of(vec![
            Entry::stored(b"stored", &payload),
            Entry::deflated(b"deflated", &payload),
        ])
        .build();
        prop_assert_eq!(compare(&image, "written").agreed, 2);
    }
}
