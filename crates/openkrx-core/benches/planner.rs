//! Planner and structural-check benchmarks, at the entry ceiling.
//!
//! Both layers here sit on top of an inventory the reader already accepted, and
//! both do work whose obvious implementation is quadratic: the extraction
//! planner folds every destination name against every other one to find a
//! collision, and the structural check resolves every `MELLEKLET` reference
//! against every archive entry. Those are the two paths a maximal-but-valid
//! package would make expensive, so they are measured at 32 and at the ceiling,
//! and the pair is what makes a change of shape visible in the report.
//!
//! Every package is built in the benchmark's own setup by the test-only writer
//! behind the `synthetic-writer` feature, and none is committed.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use openkrx_core::extract::{self, ExtractLimits};
use openkrx_core::synthetic::{Archive, Entry, meta};
use openkrx_core::{Limits, MetadataLimits, archive, profile};

/// Entry counts measured, up to `Limits::DEFAULT.max_entries`.
const ENTRY_COUNTS: [usize; 2] = [32, 256];
/// Attachment counts measured.
///
/// The ceiling is `max_entries` (256) less the format marker and the metadata
/// document, so 254 attachments is a full package and not an arbitrary number.
const ATTACHMENT_COUNTS: [usize; 2] = [32, 254];
/// Decoded bytes each payload entry carries; small, so that inflate stays out
/// of a measurement about counts.
const PAYLOAD_BYTES: usize = 512;

// --------------------------------------------------------------- the packages

/// A destination name at `index`, long and non-ASCII on purpose.
///
/// `Limits::DEFAULT.max_name_bytes` is 255 and the planner folds each component
/// to NFC and then case-folds it, so a name that is long, multi-byte and
/// decomposable is the expensive one to compare. The index suffix keeps every
/// folded name distinct, so the planner completes rather than reporting a
/// collision.
fn long_unicode_name(index: usize) -> String {
    let name = format!(
        "KRX/OCD/Payload/ID-{}/{}-{index}.bin",
        index + 1,
        "árvíztűrőtükörfúrógép".repeat(6)
    );
    assert!(
        name.len() as u64 <= Limits::DEFAULT.max_name_bytes.into(),
        "the benchmark name is over max_name_bytes"
    );
    name
}

/// An image of `count` stored entries carrying long Unicode names.
fn unicode_image(count: usize) -> Vec<u8> {
    let payload = openkrx_core::synthetic::pseudo_random(PAYLOAD_BYTES);
    let entries = (0..count)
        .map(|index| Entry::stored(long_unicode_name(index).as_bytes(), &payload))
        .collect();
    Archive::of(entries).build()
}

/// A KRX-shaped package carrying `count` attachments, each referenced.
///
/// The document lists exactly the entries the archive holds, in the documented
/// layout, so every reference resolves and the check runs its whole inventory
/// rather than stopping at a missing one.
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

// ------------------------------------------------------------- the benchmarks

/// `extract::plan` over entries with long Unicode names.
fn extract_plan(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("extract_plan");
    for count in ENTRY_COUNTS {
        let image = unicode_image(count);
        let inventory = archive::inventory(&image, &Limits::DEFAULT)
            .expect("the benchmark image is a readable archive");
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("unicode_names", count),
            &inventory,
            |bencher, inventory| {
                bencher.iter(|| {
                    extract::plan(black_box(inventory), &ExtractLimits::DEFAULT)
                        .expect("the benchmark package plans")
                });
            },
        );
    }
    group.finish();
}

/// `profile::check` over a package whose every attachment is referenced.
fn profile_check(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("profile_check");
    for count in ATTACHMENT_COUNTS {
        let image = referenced_package(count);
        let inventory = archive::inventory(&image, &Limits::DEFAULT)
            .expect("the benchmark image is a readable archive");
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("attachments", count),
            &inventory,
            |bencher, inventory| {
                bencher.iter(|| {
                    profile::check(black_box(inventory), &MetadataLimits::DEFAULT)
                        .expect("the benchmark package's metadata entry re-reads")
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, extract_plan, profile_check);
criterion_main!(benches);
