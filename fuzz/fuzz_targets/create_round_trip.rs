#![no_main]

use std::collections::BTreeSet;
use std::sync::LazyLock;

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use openkrx_core::create::{self, AttachmentInput, FixedTimestamp, Layout, PackageSpec};
use openkrx_core::metadata::{ConsignmentKind, SourceSystem};
use openkrx_core::{Limits, MetadataLimits, archive, draft, profile};

/// The `create.*` codes `docs/codes.md` catalogues, read from the document
/// itself so the two can never drift apart.
///
/// The scan is deliberately crude: every backtick-delimited span of the
/// document that spells three dot-separated lower-case segments beginning with
/// `create.`. That admits the table rows and the prose mentions alike, and
/// admits nothing else — `create.over_limit.*` carries a `*`, and a path such
/// as `create.rs` has two segments.
static DOCUMENTED_CODES: LazyLock<BTreeSet<&'static str>> = LazyLock::new(|| {
    include_str!("../../docs/codes.md")
        .split('`')
        .skip(1)
        .step_by(2)
        .filter(|span| {
            let mut segments = span.split('.');
            let head = segments.next() == Some("create");
            let shape = |segment: &str| {
                !segment.is_empty()
                    && segment.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
                    })
            };
            head && segments.clone().count() == 2 && segments.all(shape)
        })
        .collect()
});

/// The longest text this target puts into any single field.
const MAX_TEXT: usize = 32;
/// The largest attachment this target builds, in bytes.
const MAX_ATTACHMENT_BYTES: usize = 4096;

/// A header value: printable, bounded, and with no whitespace at either edge.
///
/// The writer refuses a control character (`create.invalid.text`) and text the
/// reader would trim (`create.invalid.untrimmed_text`), so filtering both here
/// is what keeps the interesting half of this target — the round trip — worth
/// reaching. File names are *not* filtered this way: their refusals are the
/// `create.unsafe_name.*` classes, which the error arm below checks.
fn header_text(bytes: &mut Unstructured<'_>) -> arbitrary::Result<String> {
    let raw = String::arbitrary(bytes)?;
    let printable: String = raw
        .chars()
        .filter(|character| !character.is_control() && !is_noncharacter(*character))
        .take(MAX_TEXT)
        .collect();
    Ok(printable.trim().to_owned())
}

/// A file name: bounded and free of control characters, nothing more.
fn file_name(bytes: &mut Unstructured<'_>) -> arbitrary::Result<String> {
    let raw = String::arbitrary(bytes)?;
    Ok(raw
        .chars()
        .filter(|character| !character.is_control() && !is_noncharacter(*character))
        .take(MAX_TEXT)
        .collect())
}

/// A code point XML 1.0 cannot carry whatever it is escaped as.
fn is_noncharacter(character: char) -> bool {
    let value = character as u32;
    (0xFDD0..=0xFDEF).contains(&value) || (value & 0xFFFE) == 0xFFFE
}

fn optional_text(bytes: &mut Unstructured<'_>) -> arbitrary::Result<Option<String>> {
    Ok(if bool::arbitrary(bytes)? {
        Some(header_text(bytes)?)
    } else {
        None
    })
}

fn spec(bytes: &mut Unstructured<'_>) -> arbitrary::Result<PackageSpec> {
    let source_systems = [
        SourceSystem::Nova,
        SourceSystem::Kir3,
        SourceSystem::Ker,
        SourceSystem::Posta,
        SourceSystem::Imap,
    ];
    let consignment_kinds = [
        ConsignmentKind::Kuldemeny,
        ConsignmentKind::Nyugta,
        ConsignmentKind::Expedialas,
        ConsignmentKind::Tertiveveny,
        ConsignmentKind::Hibajelzes,
    ];
    let header = draft::HeaderDraft {
        version: header_text(bytes)?,
        source_system: *bytes.choose(&source_systems)?,
        consignment_id: header_text(bytes)?,
        created_at_text: header_text(bytes)?,
        consignment_kind: *bytes.choose(&consignment_kinds)?,
        test: bool::arbitrary(bytes)?,
        barcode: optional_text(bytes)?,
        reference_id: optional_text(bytes)?,
        error_code: optional_text(bytes)?,
        note: optional_text(bytes)?,
    }
    .build();

    // Zero, one or two `EXPEDIALAS` blocks: attachments need exactly one, and
    // the other two counts are what `create.invalid.dispatch_count` refuses.
    let dispatch_count = bytes.int_in_range(0..=2_u8)?;
    let dispatches = (0..dispatch_count)
        .map(|_| {
            // `-1` stands for "the caller declared nothing"; every other value
            // is an assertion about the attachments, checked and possibly
            // refused as `create.invalid.reference_mismatch`.
            let declared = bytes.int_in_range(-1..=5_i64)?;
            Ok(draft::dispatch((declared >= 0).then_some(declared)))
        })
        .collect::<arbitrary::Result<Vec<_>>>()?;

    let attachment_count = bytes.int_in_range(0..=4_u8)?;
    let attachments = (0..attachment_count)
        .map(|_| {
            let mut data = Vec::<u8>::arbitrary(bytes)?;
            data.truncate(MAX_ATTACHMENT_BYTES);
            Ok(AttachmentInput {
                file_name: file_name(bytes)?,
                bytes: data,
                description: optional_text(bytes)?,
                quantity: optional_text(bytes)?,
                quantity_unit: optional_text(bytes)?,
            })
        })
        .collect::<arbitrary::Result<Vec<_>>>()?;

    Ok(PackageSpec {
        metadata: draft::metadata(header, dispatches),
        attachments,
        // Both fields are written verbatim, so every value is in range by
        // construction and `create.invalid.timestamp` is out of reach here.
        timestamp: FixedTimestamp::from_dos(u16::arbitrary(bytes)?, u16::arbitrary(bytes)?),
        layout: Layout::CanonicalDocumented,
    })
}

// The writer's promise, held against an arbitrary request: what it writes, its
// own reader accepts. On success the package must read back with no failing
// structural check and every attachment byte-identical — the invariant. On
// refusal the diagnostic must be one `docs/codes.md` catalogues, so a caller
// never meets a `create.*` code that is not documented.
fuzz_target!(|data: &[u8]| {
    let mut bytes = Unstructured::new(data);
    let Ok(spec) = spec(&mut bytes) else {
        return;
    };
    match create::package(&spec, &Limits::DEFAULT) {
        Ok(image) => {
            let report =
                create::verify_round_trip(&image, &Limits::DEFAULT, &MetadataLimits::DEFAULT)
                    .expect("a package the writer produced must read back as an archive");
            assert!(
                report
                    .checks()
                    .iter()
                    .all(|check| !matches!(check.outcome, profile::CheckOutcome::Fail(_))),
                "a package the writer produced fails one of its own structural checks"
            );
            let inventory = archive::inventory(&image, &Limits::DEFAULT)
                .expect("a package the writer produced must be an inventory");
            for (position, attachment) in spec.attachments.iter().enumerate() {
                // The marker and the metadata document come first, then the
                // attachments in the order they were given.
                let index = (position + 2) as u32;
                let written = inventory
                    .entry_bytes(index)
                    .expect("every attachment must be readable back");
                assert_eq!(
                    written, attachment.bytes,
                    "an attachment did not read back byte-identically"
                );
            }
        }
        Err(error) => {
            let code = error.code();
            assert!(
                code.starts_with("create."),
                "the writer reported a code outside its own namespace"
            );
            assert!(
                DOCUMENTED_CODES.contains(code),
                "the writer reported a code docs/codes.md does not catalogue"
            );
        }
    }
});
