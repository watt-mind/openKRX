//! Writer benchmark: one whole package written from a typed request.
//!
//! `create::package` derives the `MELLEKLET` references, deflates every
//! attachment, and assembles the image, so one measurement covers the whole
//! write. It is measured at 1 MiB and at 16 MiB of attachment bytes: one number
//! alone says nothing about shape, and the pair does.
//!
//! The request is built in the benchmark's own setup and nothing is committed.
//! The attachment bytes are a deterministic xorshift stream, which deflate
//! cannot shrink — that keeps the writer's compression-ratio ceiling out of the
//! measurement and keeps the image the size the label says.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use openkrx_core::create::{self, AttachmentInput, PackageSpec};
use openkrx_core::metadata::Metadata;
use openkrx_core::synthetic::meta;
use openkrx_core::{Limits, MetadataLimits, metadata};

/// One mebibyte.
const MIB: usize = 1024 * 1024;
/// Total attachment bytes measured.
const SPEC_SIZES: [usize; 2] = [MIB, 16 * MIB];
/// Attachments each request carries, fixed so size is the only variable.
const ATTACHMENTS: usize = 16;

/// The document a caller starts from: a header and one empty dispatch block.
///
/// The writer needs exactly one `EXPEDIALAS` block to place its derived
/// references in, and it refuses a request carrying attachments with none.
fn metadata_with_dispatch() -> Metadata {
    let document = meta::Document {
        declared_count: None,
        attachments: Vec::new(),
        handling_instructions: true,
        ..meta::Document::default()
    };
    metadata::parse(&document.bytes(), &MetadataLimits::DEFAULT)
        .expect("the benchmark document parses")
}

/// A request carrying about `bytes` of attachment content in total.
fn spec(bytes: usize) -> PackageSpec {
    let per_attachment = bytes / ATTACHMENTS;
    let attachments = (0..ATTACHMENTS)
        .map(|index| {
            AttachmentInput::described(
                format!("melleklet-{index}.bin"),
                openkrx_core::synthetic::pseudo_random(per_attachment),
                format!("árvíztűrő melléklet {index}"),
            )
        })
        .collect();
    PackageSpec::with_attachments(metadata_with_dispatch(), attachments)
}

/// `create::package` over a request at each measured size.
fn create_package(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("create_package");
    // One iteration deflates up to 16 MiB, so the default hundred samples would
    // cost minutes without saying anything the ten do not.
    group.sample_size(10);
    for bytes in SPEC_SIZES {
        let spec = spec(bytes);
        group.throughput(Throughput::Bytes(bytes as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}MiB", bytes / MIB)),
            &spec,
            |bencher, spec| {
                bencher.iter(|| {
                    create::package(black_box(spec), &Limits::DEFAULT)
                        .expect("the benchmark request is one the writer accepts")
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, create_package);
criterion_main!(benches);
