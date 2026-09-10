//! Reader benchmarks: the inventory pass, one re-decoded entry, and parsing.
//!
//! Every package measured here is built in the benchmark's own setup by the
//! test-only writer behind the `synthetic-writer` feature, and none is
//! committed: `docs/profile.md` records that no redistribution licence exists
//! for the primary sources, and a benchmark corpus is not an exception to that.
//!
//! The sizes are the documented ceilings in `Limits::DEFAULT` and
//! `MetadataLimits::DEFAULT`, because a ceiling is where a superlinear path
//! costs the most. `inventory` is measured over stored and over deflated
//! entries separately, so that the cost of inflate is visible beside the cost
//! of walking the structure rather than mixed into one number.
//!
//! These numbers are informational. The assertion that no path here is
//! superlinear lives in `tests/scaling_guard.rs`, which fails CI; a benchmark
//! only makes a change visible to a human reading its report.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use openkrx_core::synthetic::{Archive, Entry, meta};
use openkrx_core::{Limits, MetadataLimits, archive, metadata};

/// One mebibyte.
const MIB: usize = 1024 * 1024;
/// Archive image sizes measured, up to `Limits::DEFAULT.max_archive_bytes`.
const IMAGE_SIZES: [usize; 3] = [MIB, 16 * MIB, 64 * MIB];
/// Entries every sized image is split over.
///
/// Fixed across the sizes so that the image size is the only variable, and
/// small enough that no entry approaches `max_entry_decoded_bytes` (32 MiB)
/// before the largest image reaches `max_archive_bytes`.
const IMAGE_ENTRIES: usize = 8;
/// Entries in the count-bound package: `Limits::DEFAULT.max_entries`.
const MAX_ENTRIES: usize = 256;
/// Decoded bytes each entry of the count-bound package carries.
const SMALL_ENTRY_BYTES: usize = 4 * 1024;
/// Bytes reserved per stored entry for its local header, name and directory
/// record, subtracted from that entry's share of a requested image size.
const STORED_OVERHEAD: usize = 256;
/// The same reservation for a deflated entry, which also has to absorb the
/// expansion deflate adds to content it cannot shrink.
const DEFLATED_OVERHEAD: usize = 8 * 1024;

// --------------------------------------------------------------- the packages

/// One entry's payload in an image of about `bytes`, less `overhead`.
///
/// `saturating_sub` rather than `-`: every size a benchmark asks for is
/// megabytes, but a smaller one added later would underflow here and panic in a
/// debug build with nothing but an arithmetic message. The assertion is what
/// rejects such a size, and it names the size and the reservation.
fn per_entry_bytes(bytes: usize, overhead: usize) -> usize {
    let per_entry = (bytes / IMAGE_ENTRIES).saturating_sub(overhead);
    assert!(
        per_entry > 0,
        "an image of {bytes} bytes leaves no payload across {IMAGE_ENTRIES} entries \
         reserving {overhead} bytes each"
    );
    per_entry
}

/// The name of the payload entry at `index`, in the documented layout.
fn payload_name(index: usize) -> String {
    format!("KRX/OCD/Payload/ID-{}/payload-{index}.bin", index + 1)
}

/// An image of about `bytes` split over [`IMAGE_ENTRIES`] stored entries.
///
/// The payload is a cheap repeating pattern rather than noise: a stored entry
/// is written verbatim, so its content changes nothing the reader does with it,
/// and generating 64 MiB of xorshift would dominate the setup.
fn stored_image(bytes: usize) -> Vec<u8> {
    let per_entry = per_entry_bytes(bytes, STORED_OVERHEAD);
    let entries = (0..IMAGE_ENTRIES)
        .map(|index| {
            let data: Vec<u8> = (0..per_entry)
                .map(|offset| (offset ^ index) as u8)
                .collect();
            Entry::stored(payload_name(index).as_bytes(), &data)
        })
        .collect();
    bounded(Archive::of(entries).build())
}

/// The same, deflated, over bytes deflate cannot shrink.
///
/// Incompressible content is what keeps the image at the size asked for and
/// keeps the decoded total under `max_total_decoded_bytes`; it also keeps the
/// reader's compression-ratio ceiling out of the measurement, which is the
/// point — this measures inflate, not a refusal.
fn deflated_image(bytes: usize) -> Vec<u8> {
    let per_entry = per_entry_bytes(bytes, DEFLATED_OVERHEAD);
    let entries = (0..IMAGE_ENTRIES)
        .map(|index| {
            let data = openkrx_core::synthetic::pseudo_random(per_entry);
            Entry::deflated(payload_name(index).as_bytes(), &data)
        })
        .collect();
    bounded(Archive::of(entries).build())
}

/// An image of [`MAX_ENTRIES`] small stored entries.
fn many_entries_image() -> Vec<u8> {
    let entries = (0..MAX_ENTRIES)
        .map(|index| {
            let data: Vec<u8> = (0..SMALL_ENTRY_BYTES)
                .map(|offset| (offset ^ index) as u8)
                .collect();
            Entry::stored(payload_name(index).as_bytes(), &data)
        })
        .collect();
    bounded(Archive::of(entries).build())
}

/// Refuse to benchmark an image the reader would reject out of hand.
fn bounded(image: Vec<u8>) -> Vec<u8> {
    assert!(
        image.len() as u64 <= Limits::DEFAULT.max_archive_bytes,
        "the benchmark image is over max_archive_bytes"
    );
    image
}

/// A document whose text nodes carry about `text_bytes` bytes in total.
///
/// The text is spread over `attachments` `MELLEKLET_LEIRASA` values rather than
/// concentrated in one, because `max_text_bytes` is a total across the document
/// and the parser's per-event work is what a long run of them exercises.
fn document(attachments: usize, text_bytes: usize) -> Vec<u8> {
    let per_attachment = text_bytes / attachments;
    let listed = (0..attachments)
        .map(|index| {
            let number = index as i64 + 1;
            let mut attachment = meta::Attachment::new(
                number,
                &format!("payload-{index}.bin"),
                &format!("KRX/OCD/Payload/ID-{number}"),
            );
            attachment.description = Some("melleklet leirasa ".repeat(per_attachment / 18));
            attachment
        })
        .collect();
    meta::Document {
        declared_count: Some(attachments.to_string()),
        attachments: listed,
        ..meta::Document::default()
    }
    .bytes()
}

// ------------------------------------------------------------- the benchmarks

/// `archive::inventory` at three image sizes, stored and deflated.
fn inventory(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("inventory");
    // One iteration reads up to 64 MiB, so the default hundred samples would
    // cost minutes without telling anyone anything the ten do not.
    group.sample_size(10);
    for bytes in IMAGE_SIZES {
        for (method, image) in [
            ("stored", stored_image(bytes)),
            ("deflated", deflated_image(bytes)),
        ] {
            group.throughput(Throughput::Bytes(image.len() as u64));
            group.bench_with_input(
                BenchmarkId::new(method, format!("{}MiB", bytes / MIB)),
                &image,
                |bencher, image| {
                    bencher.iter(|| {
                        archive::inventory(black_box(image), &Limits::DEFAULT)
                            .expect("the benchmark image is a readable archive")
                    });
                },
            );
        }
    }

    let image = many_entries_image();
    group.throughput(Throughput::Elements(MAX_ENTRIES as u64));
    group.bench_with_input(
        BenchmarkId::new("stored", format!("{MAX_ENTRIES}_entries")),
        &image,
        |bencher, image| {
            bencher.iter(|| {
                archive::inventory(black_box(image), &Limits::DEFAULT)
                    .expect("the benchmark image is a readable archive")
            });
        },
    );
    group.finish();
}

/// `ArchiveInventory::entry_bytes` over every entry of a full package.
///
/// The inventory is built in the setup: what is measured is the cost of
/// re-decoding each entry a caller asks for, which is the work `extract` and
/// `profile::check` do on top of a reading pass they already paid for.
fn entry_bytes(criterion: &mut Criterion) {
    let image = many_entries_image();
    let inventory = archive::inventory(&image, &Limits::DEFAULT)
        .expect("the benchmark image is a readable archive");
    let mut group = criterion.benchmark_group("entry_bytes");
    group.throughput(Throughput::Elements(MAX_ENTRIES as u64));
    group.bench_function(format!("{MAX_ENTRIES}_entries"), |bencher| {
        bencher.iter(|| {
            for index in 0..MAX_ENTRIES as u32 {
                black_box(
                    inventory
                        .entry_bytes(black_box(index))
                        .expect("the entry was in the inventory"),
                );
            }
        });
    });
    group.finish();
}

/// `metadata::parse` on a small document and on one at the text ceiling.
fn metadata_parse(criterion: &mut Criterion) {
    let small = document(2, 10 * 1024);
    // Just under `MetadataLimits::DEFAULT.max_text_bytes` (1 MiB), spread over
    // enough attachments to stay well inside `max_elements` (10 000).
    let large = document(200, 1000 * 1000);
    let mut group = criterion.benchmark_group("metadata_parse");
    for (label, bytes) in [("10KiB", &small), ("near_max_text_bytes", &large)] {
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            bytes,
            |bencher, bytes| {
                bencher.iter(|| {
                    metadata::parse(black_box(bytes), &MetadataLimits::DEFAULT)
                        .expect("the benchmark document parses")
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, inventory, entry_bytes, metadata_parse);
criterion_main!(benches);
