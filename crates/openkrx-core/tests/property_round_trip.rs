//! Property-based round trips between the deterministic writer and the reader.
//!
//! The named tests in `create_package.rs` and `create_rejects.rs` pin the
//! examples worth reading. These five properties assert the same contract over
//! whatever the strategies in `support/strategies.rs` produce, which is the
//! cheap way to find the case nobody thought to write down:
//!
//! - **round trip** — a written package reads back as what was written: the
//!   attachment bytes byte for byte, the document as the documented normalised
//!   form of the input, and no failing structural check;
//! - **determinism** — one request, written twice, is the same bytes;
//! - **mutation** — a single changed byte is refused or read back inside the
//!   limits, and never panics;
//! - **limits** — a request over one documented ceiling is refused with that
//!   ceiling's `create.over_limit.*` code;
//! - **planning** — every name the writer writes is one `extract::plan`
//!   accepts.
//!
//! Every document and every byte here is generated in this repository. No
//! official sample is copied: `docs/profile.md` records that no redistribution
//! licence exists for the primary sources.
//!
//! The case count is 64 per property, overridable with `PROPTEST_CASES`; a
//! shrunk failing case is persisted under `proptest-regressions/`. Both are
//! described in `docs/testing.md`.

#[path = "support/strategies.rs"]
mod strategies;

use openkrx_core::create::{self, AttachmentInput, PackageSpec};
use openkrx_core::extract::{self, ExtractLimits};
use openkrx_core::metadata::Metadata;
use openkrx_core::profile::{CheckOutcome, StructureSummary};
use openkrx_core::{Limits, MetadataLimits, archive, metadata};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

use strategies::LimitCase;

/// Turn a crate diagnostic into a test-case failure.
///
/// Every error type here prints its stable code and numbers and no package
/// content, so the message a failing case reports stays as safe to read as a
/// logged diagnostic.
fn fail(error: impl core::fmt::Display) -> TestCaseError {
    TestCaseError::fail(error.to_string())
}

// ------------------------------------------------------------- (a) round trip

proptest! {
    #![proptest_config(strategies::config("proptest-regressions/round_trip.txt"))]

    /// A written package reads back as exactly what was asked for.
    #[test]
    fn a_written_package_reads_back_as_what_was_written(spec in strategies::spec()) {
        let image = create::package(&spec, &Limits::DEFAULT).map_err(fail)?;
        let inventory = archive::inventory(&image, &Limits::DEFAULT).map_err(fail)?;
        prop_assert_eq!(inventory.len(), spec.attachments.len() + 2);

        let document = inventory.entry_bytes(1).map_err(fail)?;
        let written = metadata::parse(&document, &MetadataLimits::DEFAULT).map_err(fail)?;
        assert_normalised(&spec.metadata, &spec.attachments, &written)?;

        for (index, attachment) in spec.attachments.iter().enumerate() {
            let bytes = inventory
                .entry_bytes(index as u32 + 2)
                .map_err(fail)?;
            prop_assert!(
                bytes == attachment.bytes,
                "attachment {index} did not read back byte-identically"
            );
        }

        let report = create::verify_round_trip(&image, &Limits::DEFAULT, &MetadataLimits::DEFAULT)
            .map_err(fail)?;
        for check in report.checks() {
            prop_assert!(
                !matches!(check.outcome, CheckOutcome::Fail(_)),
                "{} reported {:?}",
                check.id.as_str(),
                check.outcome
            );
        }
        // A19 leaves the `KRX/OCD/` marker prefix open and M13 the unit of
        // `MERET`, so a package this writer produces is never more than this.
        prop_assert_eq!(report.summary(), StructureSummary::Unresolved);

        // The normalised document is a fixed point: handing it back with the
        // same attachments writes the same bytes, so nothing drifts on a
        // second pass and the derived references are exactly the ones the
        // writer accepts as caller-supplied.
        let again = PackageSpec { metadata: written, ..spec.clone() };
        let rewritten = create::package(&again, &Limits::DEFAULT).map_err(fail)?;
        prop_assert!(rewritten == image, "the writer is not a fixed point");
    }
}

/// Assert the document read back is the documented normalised form of the input.
///
/// "Normalised" is one derivation, and only one: the writer replaces the single
/// dispatch's `MELLEKLET` list and `MELLEKLETEK_SZAMA` with values derived from
/// the attachments actually written — the 1-based number, the file name, the
/// `KRX/OCD/Payload/ID-<n>` location and `MERET` in kilobytes rounded up (M6).
/// Everything else — the header, the three marker blocks, the unqualified
/// `KEZELESI_UTASITASOK` flag — comes back unchanged.
fn assert_normalised(
    input: &Metadata,
    attachments: &[AttachmentInput],
    written: &Metadata,
) -> Result<(), TestCaseError> {
    prop_assert_eq!(&written.header, &input.header);
    prop_assert_eq!(written.receipt_present, input.receipt_present);
    prop_assert_eq!(written.openings_present, input.openings_present);
    prop_assert_eq!(written.return_receipt_present, input.return_receipt_present);
    prop_assert_eq!(written.unknown_elements, 0);
    prop_assert_eq!(written.dispatches.len(), input.dispatches.len());

    let Some(dispatch) = written.dispatches.first() else {
        prop_assert!(attachments.is_empty());
        return Ok(());
    };
    prop_assert_eq!(
        dispatch.handling_instructions_unqualified,
        input.dispatches[0].handling_instructions_unqualified
    );
    prop_assert_eq!(
        dispatch.declared_attachment_count,
        Some(attachments.len() as i64)
    );
    prop_assert_eq!(dispatch.attachments.len(), attachments.len());
    for (index, (reference, attachment)) in dispatch.attachments.iter().zip(attachments).enumerate()
    {
        let kilobytes = attachment.bytes.len().div_ceil(1024);
        prop_assert_eq!(reference.number, index as i64 + 1);
        prop_assert_eq!(&reference.file_name, &attachment.file_name);
        let location = format!("KRX/OCD/Payload/ID-{}", index + 1);
        let size_text = kilobytes.to_string();
        prop_assert_eq!(&reference.location, &location);
        prop_assert_eq!(&reference.size_text, &size_text);
        prop_assert_eq!(reference.size_value, Some(kilobytes as f64));
        prop_assert_eq!(&reference.description, &attachment.description);
        prop_assert_eq!(&reference.quantity, &attachment.quantity);
        prop_assert_eq!(&reference.quantity_unit, &attachment.quantity_unit);
    }
    Ok(())
}

// ------------------------------------------------------------ (b) determinism

proptest! {
    #![proptest_config(strategies::config("proptest-regressions/determinism.txt"))]

    /// The same request always writes the same bytes.
    #[test]
    fn the_same_request_always_writes_the_same_bytes(spec in strategies::spec()) {
        let first = create::package(&spec, &Limits::DEFAULT).map_err(fail)?;
        let second = create::package(&spec, &Limits::DEFAULT).map_err(fail)?;
        prop_assert!(
            first == second,
            "two writes of one request differ ({} and {} bytes)",
            first.len(),
            second.len()
        );
    }
}

// --------------------------------------------------------------- (c) mutation

proptest! {
    #![proptest_config(strategies::config("proptest-regressions/mutation.txt"))]

    /// One changed byte is refused, or read back inside the limits.
    ///
    /// The property is not that a mutation is always detected — a byte inside
    /// an unused header field need not be — but that the reader either refuses
    /// the image or stays inside every ceiling it was given, and that no path
    /// through it panics. Nothing is unwrapped: every call is matched, so a
    /// panic can only come from the crate under test.
    #[test]
    fn a_single_byte_mutation_is_refused_or_read_within_the_limits(
        spec in strategies::small_spec(),
        position in any::<prop::sample::Index>(),
        value in any::<u8>(),
    ) {
        let image = create::package(&spec, &Limits::DEFAULT).map_err(fail)?;
        let mut mutated = image.clone();
        let at = position.index(mutated.len());
        // `value | 1` guarantees at least one bit moves, so every case really
        // is a mutation rather than a rewrite of the same byte.
        mutated[at] ^= value | 1;
        prop_assert!(mutated != image);

        let Ok(inventory) = archive::inventory(&mutated, &Limits::DEFAULT) else {
            return Ok(());
        };
        prop_assert!(inventory.len() as u64 <= u64::from(Limits::DEFAULT.max_entries));
        let mut total = 0_u64;
        for index in 0..inventory.len() as u32 {
            if let Ok(bytes) = inventory.entry_bytes(index) {
                prop_assert!(bytes.len() as u64 <= Limits::DEFAULT.max_entry_decoded_bytes);
                total = total.saturating_add(bytes.len() as u64);
            }
        }
        prop_assert!(total <= Limits::DEFAULT.max_total_decoded_bytes);
        // Both readers run over the mutated image for the same reason: whatever
        // they report, they must report it as a value.
        let _ = create::verify_round_trip(&mutated, &Limits::DEFAULT, &MetadataLimits::DEFAULT);
        let _ = extract::plan(&inventory, &ExtractLimits::DEFAULT);
    }
}

// ----------------------------------------------------------------- (d) limits

proptest! {
    #![proptest_config(strategies::config("proptest-regressions/limits.txt"))]

    /// A request over one documented ceiling is refused with that code.
    #[test]
    fn a_request_over_one_ceiling_is_refused_with_its_code(
        spec in strategies::small_spec(),
        case in strategies::limit_case(),
    ) {
        let (spec, limits, expected) = tighten(spec, case)?;
        match create::package(&spec, &limits) {
            Ok(image) => prop_assert!(
                false,
                "{expected} was expected, but {} bytes were written",
                image.len()
            ),
            Err(error) => prop_assert_eq!(error.code(), expected),
        }
    }
}

/// Tighten exactly one ceiling to just below what this request needs.
///
/// The ceiling is derived from the package the request actually writes rather
/// than guessed, so the refusal is about the one limit under test and never
/// about a second one that happened to bind first.
fn tighten(
    mut spec: PackageSpec,
    case: LimitCase,
) -> Result<(PackageSpec, Limits, &'static str), TestCaseError> {
    let mut limits = Limits::DEFAULT;
    if case == LimitCase::CompressionRatio {
        // A run of zeros one byte past the grace: the ratio is in the hundreds,
        // and the ceiling is the reader's own, checked after compression.
        spec.attachments[0].bytes = vec![0_u8; Limits::RATIO_GRACE_BYTES as usize + 1];
        limits.max_compression_ratio = 1;
        return Ok((spec, limits, "create.over_limit.compression_ratio"));
    }
    let image = create::package(&spec, &limits).map_err(fail)?;
    let inventory = archive::inventory(&image, &limits).map_err(fail)?;
    let longest_name = inventory
        .entries()
        .iter()
        .map(|entry| entry.name_bytes().len() as u64)
        .max()
        .unwrap_or_default();
    let largest_entry = inventory
        .entries()
        .iter()
        .map(archive::ArchiveEntry::decoded_size)
        .max()
        .unwrap_or_default();
    let total: u64 = inventory
        .entries()
        .iter()
        .map(archive::ArchiveEntry::decoded_size)
        .sum();
    let code = match case {
        LimitCase::Entries => {
            limits.max_entries = inventory.len() as u32 - 1;
            "create.over_limit.entries"
        }
        LimitCase::NameBytes => {
            limits.max_name_bytes = longest_name as u32 - 1;
            "create.over_limit.name_bytes"
        }
        LimitCase::EntryBytes => {
            limits.max_entry_decoded_bytes = largest_entry - 1;
            "create.over_limit.entry_bytes"
        }
        LimitCase::TotalBytes => {
            limits.max_total_decoded_bytes = total - 1;
            "create.over_limit.total_bytes"
        }
        LimitCase::ArchiveBytes => {
            limits.max_archive_bytes = image.len() as u64 - 1;
            "create.over_limit.archive_bytes"
        }
        LimitCase::CompressionRatio => unreachable!("handled above"),
    };
    Ok((spec, limits, code))
}

// ---------------------------------------------------------------- (e) planning

proptest! {
    #![proptest_config(strategies::config("proptest-regressions/planning.txt"))]

    /// Every name the writer accepts, the extraction planner accepts too.
    #[test]
    fn the_planner_accepts_every_name_the_writer_writes(spec in strategies::spec()) {
        let image = create::package(&spec, &Limits::DEFAULT).map_err(fail)?;
        let inventory = archive::inventory(&image, &Limits::DEFAULT).map_err(fail)?;
        let plan = extract::plan(&inventory, &ExtractLimits::DEFAULT).map_err(fail)?;
        // No directory entry is written, so the plan holds exactly the entries.
        prop_assert_eq!(plan.items().len(), inventory.len());
        for item in plan.items() {
            let entry = &inventory.entries()[item.entry_index() as usize];
            let name = core::str::from_utf8(entry.name_bytes()).map_err(fail)?;
            let joined = item.components().join("/");
            prop_assert_eq!(joined.as_str(), name);
            prop_assert_eq!(item.decoded_size(), entry.decoded_size());
        }
    }
}
