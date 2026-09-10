//! A guard against a superlinear path at the documented input ceilings.
//!
//! Every value in `Limits::DEFAULT` is a ceiling, and a maximal-but-valid
//! package is therefore something a caller can be handed at any time. The
//! benchmarks under `benches/` make the cost of that package visible to a human
//! reading a report; this file is the part that fails CI. It measures the same
//! operation at a small size and at the ceiling and asserts that the cost grew
//! no faster than a generous multiple of the size.
//!
//! Three paths are guarded, each chosen because its obvious implementation is
//! quadratic:
//!
//! - `archive::inventory` at 4 MiB and 64 MiB, over **stored** entries, so that
//!   the cost of inflate is excluded and what is left is the structural walk,
//!   the CRC and the exact-coverage check;
//! - `extract::plan` at 32 and 256 entries, whose collision detection folds
//!   every destination name against the ones already planned;
//! - `profile::check` at 32 and 254 attachments — 254 is `max_entries` less the
//!   format marker and the metadata document, so it is a full package — whose
//!   reference resolution matches every `MELLEKLET` against every entry.
//!
//! ## The thresholds, and why they are so loose
//!
//! Each assertion allows a wall-time ratio of [`MAX_RATIO`] (32) across a size
//! ratio of 16 (4 MiB to 64 MiB) or 8 (32 to 256). Linear would be 16 and 8;
//! quadratic would be 256 and 64. The threshold therefore sits at about twice
//! linear and well below quadratic: it catches a change of *shape* and is
//! deliberately blind to a change of constant factor, because a shared CI
//! runner, a debug build and a cold cache all move the constant and none of
//! them moves the exponent. A guard that flakes gets disabled, and a disabled
//! guard catches nothing.
//!
//! Two further things keep the numbers honest. Every size is measured
//! [`REPEATS`] times and the **minimum** is taken, because noise only ever adds
//! time. And the small size is repeated [`SMALL_REPEATS`] times inside one
//! measurement where a single call would be too short to time reliably; the
//! large size is repeated the same number of times, so the repetition cancels
//! out of the ratio entirely.
//!
//! The whole file is built to run in a debug build inside CI's budget: it stays
//! well under 30 seconds there, which is why the sizes are what they are and why
//! nothing here is deflated.

mod support;

use std::hint::black_box;
use std::time::{Duration, Instant};

use openkrx_core::extract::{self, ExtractLimits};
use openkrx_core::{Limits, MetadataLimits, archive, profile};
use support::{Archive, Entry, meta};

/// Largest wall-time ratio any guarded operation may show across its sizes.
///
/// See the module documentation: linear is 16 for the byte guard and 8 for the
/// two count guards, so this is roughly twice linear in both cases.
const MAX_RATIO: f64 = 32.0;
/// Times each size is measured; the fastest run is the one compared.
const REPEATS: usize = 2;
/// Times an operation is repeated inside one measurement.
///
/// Applied to both sizes, so it cancels out of the ratio and only buys a
/// measurement long enough for the clock to resolve.
const SMALL_REPEATS: usize = 20;

/// One mebibyte.
const MIB: usize = 1024 * 1024;
/// Entries every sized image is split over, fixed so size is the only variable.
const IMAGE_ENTRIES: usize = 8;
/// Decoded bytes each counted entry carries.
const PAYLOAD_BYTES: usize = 512;
/// Bytes reserved per entry for its local header, name and directory record.
///
/// Subtracted from each entry's share of a requested image size, so the image
/// built for that size lands under it rather than over it.
const ENTRY_OVERHEAD: usize = 256;

// ------------------------------------------------------------- measuring

/// The fastest of [`REPEATS`] runs of `work`.
///
/// The minimum rather than the mean: a scheduler, another test binary on the
/// same runner or a cold cache can only ever make a run slower, so the smallest
/// observation is the closest one to the cost of the code itself.
fn fastest(mut work: impl FnMut()) -> Duration {
    (0..REPEATS)
        .map(|_| {
            let start = Instant::now();
            work();
            start.elapsed()
        })
        .min()
        .expect("REPEATS is not zero")
}

/// Assert that `large` grew no faster than [`MAX_RATIO`] times `small`.
///
/// The message names the operation, both durations and the ratio, and no
/// package content: a failure here is read from a CI log like any other
/// diagnostic in this repository.
#[track_caller]
fn assert_scales(operation: &str, small: Duration, large: Duration) {
    // A measurement that resolved to nothing would divide by zero and, worse,
    // would pass every threshold. One nanosecond is the floor, and a run that
    // reaches it is reported rather than silently accepted.
    let small_nanos = small.as_secs_f64().max(1e-9);
    let ratio = large.as_secs_f64() / small_nanos;
    // Printed so that `--nocapture` shows how much headroom a passing run had.
    // The numbers are timings on whatever runner this was, never package
    // content, so they are as safe to read from a log as any other diagnostic.
    println!("{operation}: {small:?} -> {large:?}, ratio {ratio:.1} (limit {MAX_RATIO:.0})");
    assert!(
        ratio < MAX_RATIO,
        "{operation} took {small:?} at the small size and {large:?} at the ceiling, \
         a ratio of {ratio:.1}, over the documented limit of {MAX_RATIO:.0}"
    );
}

// ------------------------------------------------------------- the packages

/// Table-driven CRC-32, matching the one the reader checks against.
///
/// The synthetic writer computes its CRC a bit at a time, which is the right
/// trade for the small archives every other test builds and the wrong one here:
/// a 64 MiB entry would spend more time being written in a debug build than the
/// guard is allowed in total. This is the same polynomial, taken a byte at a
/// time, and `archive_inventory.rs` is what holds the writer's own version
/// honest.
fn crc32(data: &[u8]) -> u32 {
    let mut table = [0_u32; 256];
    for (index, slot) in table.iter_mut().enumerate() {
        let mut value = index as u32;
        for _ in 0..8 {
            value = if value & 1 == 1 {
                0xedb8_8320 ^ (value >> 1)
            } else {
                value >> 1
            };
        }
        *slot = value;
    }
    let mut state = !0_u32;
    for byte in data {
        state = table[usize::from((state as u8) ^ *byte)] ^ (state >> 8);
    }
    !state
}

/// A stored entry holding `data`, built without the bit-at-a-time CRC.
fn stored(name: &str, data: Vec<u8>) -> Entry {
    let mut entry = Entry::stored(name.as_bytes(), b"");
    entry.uncompressed_size = u32::try_from(data.len()).expect("the entry fits in u32");
    entry.crc = crc32(&data);
    entry.data = data;
    entry
}

/// An image of about `bytes`, split over [`IMAGE_ENTRIES`] stored entries.
///
/// The payload is a cheap repeating pattern: a stored entry is written
/// verbatim, so its content changes nothing the reader does with it, and
/// generating 64 MiB of anything more interesting would cost more than the
/// measurement.
fn stored_image(bytes: usize) -> Vec<u8> {
    // `saturating_sub` rather than `-`: the sizes below are megabytes, but a
    // smaller one passed in some later revision would underflow here and panic
    // in a debug build with nothing but an arithmetic message. The assertion is
    // what rejects such a size, and it says which size it was.
    let per_entry = (bytes / IMAGE_ENTRIES).saturating_sub(ENTRY_OVERHEAD);
    assert!(
        per_entry > 0,
        "a guard image of {bytes} bytes leaves no payload across {IMAGE_ENTRIES} entries"
    );
    let entries = (0..IMAGE_ENTRIES)
        .map(|index| {
            let data = vec![index as u8; per_entry];
            stored(
                &format!("KRX/OCD/Payload/ID-{}/payload-{index}.bin", index + 1),
                data,
            )
        })
        .collect();
    let image = Archive::of(entries).build();
    assert!(
        image.len() as u64 <= Limits::DEFAULT.max_archive_bytes,
        "the guard image is over max_archive_bytes"
    );
    image
}

/// An image of `count` stored entries carrying long, multi-byte names.
///
/// Long and non-ASCII because the planner folds each component to NFC and then
/// case-folds it, which is the comparison a collision check repeats; the index
/// suffix keeps every folded name distinct so the plan completes.
fn unicode_image(count: usize) -> Vec<u8> {
    let entries = (0..count)
        .map(|index| {
            let name = format!(
                "KRX/OCD/Payload/ID-{}/{}-{index}.bin",
                index + 1,
                "árvíztűrőtükörfúrógép".repeat(6)
            );
            assert!(
                name.len() as u64 <= Limits::DEFAULT.max_name_bytes.into(),
                "the guard name is over max_name_bytes"
            );
            stored(&name, vec![index as u8; PAYLOAD_BYTES])
        })
        .collect();
    Archive::of(entries).build()
}

/// A KRX-shaped package whose document references every attachment it holds.
fn referenced_package(count: usize) -> Vec<u8> {
    let names: Vec<String> = (0..count)
        .map(|index| format!("KRX/OCD/Payload/ID-{}/melleklet-{index}.bin", index + 1))
        .collect();
    let listed = (0..count)
        .map(|index| {
            meta::Attachment::new(
                index as i64 + 1,
                &format!("melleklet-{index}.bin"),
                &format!("KRX/OCD/Payload/ID-{}", index + 1),
            )
        })
        .collect();
    let document = meta::Document {
        declared_count: Some(count.to_string()),
        attachments: listed,
        ..meta::Document::default()
    };
    let payloads: Vec<&str> = names.iter().map(String::as_str).collect();
    meta::krx(
        "KRX/OCD/",
        meta::METADATA_FILE,
        &document.bytes(),
        &payloads,
    )
}

/// The inventory of `image`, which every guarded package is built to produce.
fn inventory(image: &[u8]) -> archive::ArchiveInventory<'_> {
    archive::inventory(image, &Limits::DEFAULT).expect("the guard image is a readable archive")
}

// ------------------------------------------------------------- the guards

/// Reading an image 16 times larger must not cost 32 times as much.
#[test]
fn inventory_scales_with_the_image_size() {
    let small = stored_image(4 * MIB);
    let large = stored_image(64 * MIB);
    let read = |image: &[u8]| {
        black_box(inventory(black_box(image)).len());
    };
    let small_time = fastest(|| read(&small));
    let large_time = fastest(|| read(&large));
    assert_scales(
        "archive::inventory over 4 MiB and 64 MiB",
        small_time,
        large_time,
    );
}

/// Planning 8 times as many entries must not cost 32 times as much.
#[test]
fn extraction_planning_scales_with_the_entry_count() {
    let small = unicode_image(32);
    let large = unicode_image(256);
    let small_inventory = inventory(&small);
    let large_inventory = inventory(&large);
    let plan = |inventory: &archive::ArchiveInventory<'_>| {
        for _ in 0..SMALL_REPEATS {
            black_box(
                extract::plan(black_box(inventory), &ExtractLimits::DEFAULT)
                    .expect("the guard package plans")
                    .len(),
            );
        }
    };
    let small_time = fastest(|| plan(&small_inventory));
    let large_time = fastest(|| plan(&large_inventory));
    assert_scales(
        "extract::plan over 32 and 256 entries",
        small_time,
        large_time,
    );
}

/// Checking 8 times as many attachments must not cost 32 times as much.
#[test]
fn structural_checks_scale_with_the_attachment_count() {
    let small = referenced_package(32);
    let large = referenced_package(254);
    let small_inventory = inventory(&small);
    let large_inventory = inventory(&large);
    let check = |inventory: &archive::ArchiveInventory<'_>| {
        for _ in 0..SMALL_REPEATS {
            black_box(
                profile::check(black_box(inventory), &MetadataLimits::DEFAULT)
                    .expect("the guard package's metadata entry re-reads")
                    .checks()
                    .len(),
            );
        }
    };
    let small_time = fastest(|| check(&small_inventory));
    let large_time = fastest(|| check(&large_inventory));
    assert_scales(
        "profile::check over 32 and 254 attachments",
        small_time,
        large_time,
    );
}
