//! Property-based tests for what repacking preserves and what it refuses.
//!
//! `repack_plan.rs` and `repack_rejects.rs` pin the examples worth reading.
//! These six properties assert the same contract over whatever the strategies
//! in `support/strategies.rs` produce, which is the cheap way to find the case
//! nobody thought to write down — repacking composes the reader, the parser
//! and the writer, so its corner cases are the products of three surfaces
//! rather than one:
//!
//! - **identity** — planning an empty [`Edits`] over a package this crate wrote
//!   and applying it writes that package's bytes again;
//! - **add and remove** — adding attachments and then removing exactly the
//!   numbers they were given restores the input byte for byte, for every input
//!   whose document already lists attachments or carries the `MELLEKLETEK`
//!   container;
//! - **the one asymmetry** — for the remaining input, an empty dispatch
//!   carrying no container, the same round trip differs by exactly that empty
//!   container and by nothing else, and is the identity from then on;
//! - **preservation** — after any valid edit list, every attachment no edit
//!   named reads back byte-identical, the result fails no structural check, and
//!   the plan reports exactly the edits it was given;
//! - **refusal** — an edit naming a number the package does not carry, or
//!   naming one twice, is refused with its documented `repack.invalid.*` code
//!   and never panics;
//! - **determinism** — planning the same package twice decides the same thing,
//!   whether that is a plan or a refusal.
//!
//! Every document and every byte here is generated in this repository. No
//! official sample is copied: `docs/profile.md` records that no redistribution
//! licence exists for the primary sources.
//!
//! The case count is 64 per property, overridable with `PROPTEST_CASES`; a
//! shrunk failing case would be persisted under `proptest-regressions/`. Both
//! are described in `docs/testing.md`.

#[path = "support/strategies.rs"]
mod strategies;

use openkrx_core::create::{self, FixedTimestamp, PackageSpec};
use openkrx_core::profile::{CheckOutcome, StructureSummary};
use openkrx_core::repack::{self, AttachmentAddition, Edits, RepackPlan};
use openkrx_core::{Limits, MetadataLimits, archive, metadata};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

/// Turn a crate diagnostic into a test-case failure.
///
/// Every error type here prints its stable code and numbers and no package
/// content, so the message a failing case reports stays as safe to read as a
/// logged diagnostic.
fn fail(error: impl core::fmt::Display) -> TestCaseError {
    TestCaseError::fail(error.to_string())
}

/// Write the request, then plan `edits` over the package it produced.
///
/// The image is returned beside the plan because [`repack::apply`] needs the
/// very inventory the plan was made from, and an inventory borrows its image.
fn write(spec: &PackageSpec) -> Result<Vec<u8>, TestCaseError> {
    create::package(spec, &Limits::DEFAULT).map_err(fail)
}

/// Plan `edits` over `image` and write the result under `timestamp`.
fn repacked(
    image: &[u8],
    edits: &Edits,
    timestamp: FixedTimestamp,
) -> Result<(RepackPlan, Vec<u8>), TestCaseError> {
    let inventory = archive::inventory(image, &Limits::DEFAULT).map_err(fail)?;
    let plan = repack::plan(
        &inventory,
        &Limits::DEFAULT,
        &MetadataLimits::DEFAULT,
        edits,
    )
    .map_err(fail)?;
    let bytes = repack::apply(&inventory, &plan, timestamp, &Limits::DEFAULT).map_err(fail)?;
    Ok((plan, bytes))
}

/// The decoded bytes of every attachment of `image`, in output order.
///
/// The layout puts the marker first and the document second, so the
/// attachments are every entry from index 2 on.
fn attachment_bytes(image: &[u8]) -> Result<Vec<Vec<u8>>, TestCaseError> {
    let inventory = archive::inventory(image, &Limits::DEFAULT).map_err(fail)?;
    let mut bytes = Vec::with_capacity(inventory.len().saturating_sub(2));
    for index in 2..inventory.len() as u32 {
        bytes.push(inventory.entry_bytes(index).map_err(fail)?);
    }
    Ok(bytes)
}

/// Assert the package fails no structural check and is exactly as undecided as
/// one the writer produced.
///
/// A19 leaves the `KRX/OCD/` marker prefix open and M13 the unit of `MERET`,
/// so `Unresolved` is the best a package this crate writes ever reaches, and a
/// repacked one must reach neither more nor less.
fn assert_writer_report(image: &[u8]) -> Result<(), TestCaseError> {
    let report = create::verify_round_trip(image, &Limits::DEFAULT, &MetadataLimits::DEFAULT)
        .map_err(fail)?;
    for check in report.checks() {
        prop_assert!(
            !matches!(check.outcome, CheckOutcome::Fail(_)),
            "{} reported {:?}",
            check.id.as_str(),
            check.outcome
        );
    }
    prop_assert_eq!(report.summary(), StructureSummary::Unresolved);
    Ok(())
}

// --------------------------------------------------------------- (a) identity

proptest! {
    #![proptest_config(strategies::config("proptest-regressions/repack_identity.txt"))]

    /// An empty edit reproduces the package it was given, byte for byte.
    ///
    /// This is the rule every `repack.unsupported.*` refusal exists to keep
    /// true, so it is the one property worth asserting over every package the
    /// writer can produce that repacking accepts at all — both `MELLEKLETEK`
    /// container forms included, now that the reader retains which one a
    /// dispatch carried instead of normalising it away.
    #[test]
    fn an_empty_edit_reproduces_the_package(spec in strategies::repackable_spec()) {
        let image = write(&spec)?;
        let (plan, repacked) = repacked(&image, &Edits::default(), spec.timestamp)?;

        prop_assert_eq!(plan.attachment_count(), spec.attachments.len());
        prop_assert_eq!(plan.preserved().len(), spec.attachments.len());
        prop_assert!(plan.changed().is_empty());
        prop_assert!(plan.removed().is_empty());
        prop_assert!(plan.added().is_empty());
        prop_assert!(plan.header_fields().is_empty());

        prop_assert!(
            repacked == image,
            "an empty edit rewrote the package ({} bytes in, {} out)",
            image.len(),
            repacked.len()
        );
    }
}

// --------------------------------------------------------- (b) add and remove

/// The empty `MELLEKLETEK` element, exactly as the writer spells it.
///
/// The prefix is the writer's own `ns2`, which the document strategy in
/// `support/strategies.rs` already spells out for the same reason: it is what
/// the bytes under test actually say.
const EMPTY_CONTAINER: &str = "<ns2:MELLEKLETEK></ns2:MELLEKLETEK>";

/// Add `additions` to `image`, then remove exactly the numbers the plan gave
/// them, and return the bytes that come back.
///
/// The numbers are the plan's own [`RepackPlan::added`], not numbers the test
/// computed: an addition takes the number the *result* gives it, and asserting
/// against a number the test guessed would test the guess.
fn add_then_remove(
    image: &[u8],
    additions: &[AttachmentAddition],
    timestamp: FixedTimestamp,
) -> Result<Vec<u8>, TestCaseError> {
    let count = attachment_bytes(image)?.len();

    let grow = Edits {
        add: additions.to_vec(),
        ..Edits::default()
    };
    let (plan, grown) = repacked(image, &grow, timestamp)?;
    let added = plan.added().to_vec();
    prop_assert_eq!(plan.attachment_count(), count + additions.len());
    prop_assert_eq!(plan.preserved().len(), count);
    prop_assert_eq!(
        &added,
        &(count as u32 + 1..=(count + additions.len()) as u32).collect::<Vec<u32>>()
    );

    // The added bytes arrived where the plan said they would.
    let grown_bytes = attachment_bytes(&grown)?;
    prop_assert_eq!(grown_bytes.len(), count + additions.len());
    for (offset, addition) in additions.iter().enumerate() {
        prop_assert!(grown_bytes[count + offset] == addition.bytes);
    }

    let shrink = Edits {
        remove: added,
        ..Edits::default()
    };
    let (_, restored) = repacked(&grown, &shrink, timestamp)?;
    Ok(restored)
}

/// The metadata document of `image`, as the bytes the archive carries.
///
/// The layout puts it at entry 1, immediately after the marker.
fn document(image: &[u8]) -> Result<String, TestCaseError> {
    let inventory = archive::inventory(image, &Limits::DEFAULT).map_err(fail)?;
    let bytes = inventory.entry_bytes(1).map_err(fail)?;
    String::from_utf8(bytes).map_err(fail)
}

proptest! {
    #![proptest_config(strategies::config("proptest-regressions/repack_add_remove.txt"))]

    /// Adding attachments and removing exactly those numbers is the identity.
    ///
    /// The input is any package whose document already lists an attachment or
    /// already carries the `MELLEKLETEK` container, which is where the round
    /// trip genuinely is an identity. The one remaining shape — an empty
    /// dispatch carrying no container — is the property below, because M7
    /// makes the container and the count separate elements and the writer must
    /// emit one to hold the added reference.
    #[test]
    fn adding_attachments_and_removing_them_again_restores_the_package(
        spec in strategies::repackable_spec_with_container(),
        additions in strategies::additions(1..=3),
    ) {
        let image = write(&spec)?;
        let restored = add_then_remove(&image, &additions, spec.timestamp)?;
        prop_assert!(
            restored == image,
            "an add and its removal did not restore the package \
             ({} bytes in, {} out)",
            image.len(),
            restored.len()
        );
    }
}

proptest! {
    #![proptest_config(strategies::config("proptest-regressions/repack_container.txt"))]

    /// A dispatch that carried no container gains one, and nothing else.
    ///
    /// This is documented behaviour rather than a defect, and it is inherent
    /// to the composition. M7 makes `MELLEKLETEK` and `MELLEKLETEK_SZAMA`
    /// separate elements, so an empty container and no container at all are
    /// different documents. The writer must emit the container to hold the
    /// added reference; the intermediate package therefore genuinely carries
    /// one; the reader retains that fact rather than normalising it away; and
    /// the later removal cannot know the original had none, because nothing in
    /// the package it is given says so.
    ///
    /// What is asserted is that the difference stops there: the marker and
    /// every attachment are byte-identical, the document differs by exactly
    /// one empty `MELLEKLETEK` element and by nothing else, the parsed
    /// documents agree on everything but `attachments_present`, and a second
    /// add-and-remove over the result is an exact identity — the asymmetry
    /// happens once and never drifts further.
    #[test]
    fn a_dispatch_that_carried_no_container_gains_one_and_nothing_else(
        spec in strategies::repackable_spec_without_container(),
        additions in strategies::additions(1..=3),
    ) {
        let image = write(&spec)?;
        let once = add_then_remove(&image, &additions, spec.timestamp)?;

        // The document is the only entry that moves.
        let before = document(&image)?;
        let after = document(&once)?;
        prop_assert!(!before.contains(EMPTY_CONTAINER));
        prop_assert_eq!(after.matches(EMPTY_CONTAINER).count(), 1);
        prop_assert_eq!(after.replace(EMPTY_CONTAINER, ""), before.clone());

        // Neither package carries an attachment, so the marker is all that is
        // left, and it is the same entry in both.
        let inventory = archive::inventory(&once, &Limits::DEFAULT).map_err(fail)?;
        prop_assert_eq!(inventory.len(), 2);
        prop_assert!(attachment_bytes(&once)?.is_empty());
        prop_assert!(
            inventory.entry_bytes(0).map_err(fail)?
                == archive::inventory(&image, &Limits::DEFAULT)
                    .map_err(fail)?
                    .entry_bytes(0)
                    .map_err(fail)?
        );

        // The parsed documents agree on everything but the container flag.
        let parsed_before = metadata::parse(before.as_bytes(), &MetadataLimits::DEFAULT)
            .map_err(fail)?;
        let mut parsed_after = metadata::parse(after.as_bytes(), &MetadataLimits::DEFAULT)
            .map_err(fail)?;
        prop_assert_eq!(parsed_after.dispatches.len(), 1);
        prop_assert!(parsed_after.dispatches[0].attachments_present);
        prop_assert!(!parsed_before.dispatches[0].attachments_present);
        parsed_after.dispatches[0].attachments_present = false;
        prop_assert_eq!(&parsed_after, &parsed_before);

        assert_writer_report(&once)?;

        // Once the container is there, the round trip is an exact identity.
        let twice = add_then_remove(&once, &additions, spec.timestamp)?;
        prop_assert!(
            twice == once,
            "the asymmetry did not settle after one pass \
             ({} bytes in, {} out)",
            once.len(),
            twice.len()
        );
    }
}

// ----------------------------------------------------------- (c) preservation

proptest! {
    #![proptest_config(strategies::config("proptest-regressions/repack_preserved.txt"))]

    /// Any valid edit leaves every attachment it did not name untouched.
    ///
    /// "Untouched" is asserted against the *input's* bytes at the number the
    /// edit spared, not against the output position: removing an attachment
    /// renumbers everything after it, which is precisely the case a test that
    /// compared positions would get wrong.
    #[test]
    fn a_valid_edit_preserves_every_attachment_it_did_not_name(
        (spec, edits) in strategies::repackable_spec_with_edits(),
    ) {
        let image = write(&spec)?;
        let (plan, out) = repacked(&image, &edits, spec.timestamp)?;

        // The numbers the input keeps, in the order the result carries them.
        let kept: Vec<u32> = (1..=spec.attachments.len() as u32)
            .filter(|number| !edits.remove.contains(number))
            .collect();
        let replaced: Vec<u32> = edits
            .replace
            .iter()
            .map(|replacement| replacement.number)
            .collect();

        let mut removed = edits.remove.clone();
        removed.sort_unstable();
        let mut changed = replaced.clone();
        changed.sort_unstable();
        prop_assert_eq!(plan.removed(), removed.as_slice());
        prop_assert_eq!(plan.changed(), changed.as_slice());
        prop_assert_eq!(plan.preserved().len(), kept.len() - changed.len());
        prop_assert_eq!(plan.added().len(), edits.add.len());
        let fields = edits.header.fields();
        prop_assert_eq!(plan.header_fields(), fields.as_slice());
        prop_assert_eq!(plan.attachment_count(), kept.len() + edits.add.len());

        let written = attachment_bytes(&out)?;
        prop_assert_eq!(written.len(), kept.len() + edits.add.len());
        for (position, number) in kept.iter().enumerate() {
            let source = &spec.attachments[*number as usize - 1];
            let expected = match edits
                .replace
                .iter()
                .find(|replacement| replacement.number == *number)
            {
                Some(replacement) => &replacement.bytes,
                None => &source.bytes,
            };
            prop_assert!(
                &written[position] == expected,
                "attachment {number} did not read back byte-identically"
            );
        }
        for (offset, addition) in edits.add.iter().enumerate() {
            prop_assert!(written[kept.len() + offset] == addition.bytes);
        }

        assert_writer_report(&out)?;
    }
}

// ---------------------------------------------------------------- (d) refusal

proptest! {
    #![proptest_config(strategies::config("proptest-regressions/repack_invalid.txt"))]

    /// An edit naming an attachment the package does not hold is refused.
    ///
    /// Nothing here is unwrapped, so a panic can only come from the crate
    /// under test, and the assertion is on the stable code rather than on the
    /// variant: the code is what a consumer buckets on.
    #[test]
    fn an_edit_naming_no_attachment_is_refused_with_its_code(
        (spec, edits, expected) in strategies::repackable_spec_with_invalid_edits(),
    ) {
        let image = write(&spec)?;
        let inventory = archive::inventory(&image, &Limits::DEFAULT).map_err(fail)?;
        match repack::plan(&inventory, &Limits::DEFAULT, &MetadataLimits::DEFAULT, &edits) {
            Ok(plan) => prop_assert!(
                false,
                "{expected} was expected, but a plan for {} attachments was made",
                plan.attachment_count()
            ),
            Err(error) => prop_assert_eq!(error.code(), expected),
        }
    }
}

// ------------------------------------------------------------ (e) determinism

proptest! {
    #![proptest_config(strategies::config("proptest-regressions/repack_determinism.txt"))]

    /// Planning the same package twice decides the same thing.
    ///
    /// The request comes from the *unrestricted* strategy, so most cases carry
    /// a marker block or an unqualified `KEZELESI_UTASITASOK` and are refused
    /// with `repack.unsupported.opaque_block` — which is the point: a refusal
    /// a caller cannot reproduce is a refusal they cannot act on.
    #[test]
    fn planning_the_same_package_twice_decides_the_same_thing(
        spec in strategies::spec(),
        edits in strategies::header_edits(),
    ) {
        let image = write(&spec)?;
        let inventory = archive::inventory(&image, &Limits::DEFAULT).map_err(fail)?;
        let edits = Edits { header: edits, ..Edits::default() };
        let plan = |()| repack::plan(&inventory, &Limits::DEFAULT, &MetadataLimits::DEFAULT, &edits);
        match (plan(()), plan(())) {
            (Ok(first), Ok(second)) => prop_assert!(first == second),
            (Err(first), Err(second)) => {
                prop_assert_eq!(first.code(), second.code());
                prop_assert_eq!(first.entry_index(), second.entry_index());
                prop_assert_eq!(first.attachment_number(), second.attachment_number());
                prop_assert!(
                    first.code().starts_with("repack.unsupported."),
                    "a package the writer produced was refused as {}",
                    first.code()
                );
            }
            (first, second) => prop_assert!(
                false,
                "planning twice disagreed: {} then {}",
                first.map_or_else(|error| error.code().to_owned(), |_| "a plan".to_owned()),
                second.map_or_else(|error| error.code().to_owned(), |_| "a plan".to_owned()),
            ),
        }
    }
}
