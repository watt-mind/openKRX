//! Structural checks over an archive inventory and its metadata document.
//!
//! [`check`] runs a fixed inventory of checks and reports each one as
//! [`CheckOutcome::Pass`], [`CheckOutcome::Fail`], [`CheckOutcome::Unresolved`]
//! or [`CheckOutcome::NotApplicable`]. The inventory, in order, is published in
//! `docs/architecture.md`.
//!
//! There is no verdict. `docs/profile.md` lists rules A19 to A22 and M11 to M15
//! as unresolved, and each of them maps to a distinct `Unresolved` outcome
//! citing the rule, never to a failure. [`StructureSummary::Consistent`] means
//! "nothing failed and nothing was left open"; it is not a statement that the
//! archive is a conforming KRX package, and it never becomes one while those
//! rules stand.
//!
//! ```
//! use openkrx_core::{Limits, MetadataLimits, archive, profile};
//!
//! # fn demo(image: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
//! let inventory = archive::inventory(image, &Limits::DEFAULT)?;
//! let report = profile::check(&inventory, &MetadataLimits::DEFAULT)?;
//! for check in report.checks() {
//!     let _ = (check.id.as_str(), check.outcome);
//! }
//! # Ok(())
//! # }
//! ```

mod locate;
mod references;
mod report;

use crate::archive::ArchiveInventory;
use crate::error::ArchiveError;
use crate::metadata::{Metadata, MetadataLimits};

pub use report::{
    AttachmentResolution, Check, CheckId, CheckOutcome, Observations, ReferenceResolution, RuleId,
    StructureReport, StructureSummary,
};

use locate::{LocateError, Located, Marker};
use report::Checks;

/// Stable codes a failing structural check reports.
///
/// They share the `metadata.` namespace with [`crate::MetadataError`] codes and
/// never collide with one, so a consumer can bucket every diagnostic this crate
/// produces by its dotted code alone.
pub mod codes {
    /// No entry has the shape of a metadata document.
    pub const MISSING: &str = "metadata.missing";
    /// More than one entry does, and nothing settles which one is meant.
    pub const AMBIGUOUS_CANDIDATES: &str = "metadata.ambiguous.multiple_candidates";
    /// No entry whose last path segment is `mimetype` (A2).
    pub const MARKER_MISSING: &str = "metadata.malformed.marker_missing";
    /// A `mimetype` entry exists but is not the archive's first entry (A2).
    pub const MARKER_NOT_FIRST: &str = "metadata.malformed.marker_not_first";
    /// The marker entry holds something other than `application/OCD+ZIP` (A2).
    pub const MARKER_CONTENT: &str = "metadata.malformed.marker_content";
    /// A declared attachment names no entry, with or without a prefix (M5).
    pub const REFERENCE_MISSING_ENTRY: &str = "metadata.reference.missing_entry";
    /// Two references share an attachment number or a declared path.
    pub const REFERENCE_DUPLICATE: &str = "metadata.reference.duplicate";
    /// `MELLEKLETEK_SZAMA` disagrees with the listed references (M7).
    pub const COUNT_MISMATCH: &str = "metadata.count_mismatch";
}

/// Why a structural check could not be run at all.
///
/// A check that runs never fails this way: an unmet expectation is a
/// [`CheckOutcome`], not an error. This type reports only that the archive
/// itself could not be read far enough to check anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProfileError {
    /// Re-reading an entry from the inventory failed.
    Archive(ArchiveError),
}

impl ProfileError {
    /// Return the stable dotted identifier for this error.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Archive(error) => error.code(),
        }
    }
}

impl core::fmt::Display for ProfileError {
    /// Print the underlying archive diagnostic, which carries no content.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Archive(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ProfileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Archive(error) => Some(error),
        }
    }
}

impl From<ArchiveError> for ProfileError {
    fn from(error: ArchiveError) -> Self {
        Self::Archive(error)
    }
}

/// Run every structural check over `inventory` and report what was observed.
///
/// The checks run in [`CheckId::ORDER`]: metadata location, metadata file name,
/// root prefix, marker entry, metadata parse, schema-optional fields, handling
/// instructions form, attachment references, attachment count, attachment
/// uniqueness, declared size. A check whose input is missing reports
/// [`CheckOutcome::NotApplicable`] rather than a failure.
///
/// # Errors
///
/// Returns [`ProfileError`] only when the archive entry holding the metadata
/// document cannot be re-read. Every profile expectation, met or unmet, is
/// reported inside the [`StructureReport`].
pub fn check(
    inventory: &ArchiveInventory<'_>,
    limits: &MetadataLimits,
) -> Result<StructureReport, ProfileError> {
    let mut checks = Checks::default();
    let mut observations = report::Observations::default();

    let located = record_location(&mut checks, inventory, &mut observations);
    record_marker(&mut checks, inventory)?;

    let Some(located) = located else {
        return Ok(StructureReport::new(
            checks.finish(),
            observations,
            Vec::new(),
            None,
        ));
    };
    let bytes = inventory.entry_bytes(located.index)?;
    let metadata = match crate::metadata::parse(&bytes, limits) {
        Ok(metadata) => {
            checks.record(CheckId::MetadataParse, CheckOutcome::Pass);
            metadata
        }
        Err(error) => {
            checks.record(CheckId::MetadataParse, CheckOutcome::Fail(error.code()));
            return Ok(StructureReport::new(
                checks.finish(),
                observations,
                Vec::new(),
                None,
            ));
        }
    };
    observe_metadata(&mut observations, &metadata);
    let attachments = references::resolve(inventory, &metadata, &located.prefix);
    record_document_checks(&mut checks, &metadata, &attachments);
    Ok(StructureReport::new(
        checks.finish(),
        observations,
        attachments,
        Some(metadata),
    ))
}

/// Record the location, file-name and root-prefix checks (A4, A19, M12).
fn record_location(
    checks: &mut Checks,
    inventory: &ArchiveInventory<'_>,
    observations: &mut Observations,
) -> Option<Located> {
    let located = match locate::locate(inventory) {
        Ok(located) => located,
        Err(error) => {
            checks.record(
                CheckId::MetadataLocation,
                CheckOutcome::Fail(match error {
                    LocateError::Missing => codes::MISSING,
                    LocateError::Ambiguous => codes::AMBIGUOUS_CANDIDATES,
                }),
            );
            return None;
        }
    };
    checks.record(CheckId::MetadataLocation, CheckOutcome::Pass);
    checks.record(
        CheckId::MetadataFileName,
        if located.canonical_name {
            CheckOutcome::Pass
        } else {
            CheckOutcome::Unresolved(RuleId::M12)
        },
    );
    checks.record(
        CheckId::RootPrefix,
        if located.prefix == b"KRX/OCD/" {
            CheckOutcome::Pass
        } else {
            CheckOutcome::Unresolved(RuleId::A19)
        },
    );
    observations.root_prefix = Some(located.prefix.clone());
    observations.metadata_entry_name = Some(located.name.clone());
    observations.metadata_entry_index = Some(located.index);
    Some(located)
}

/// Record the marker-entry check (A2), leaving A19 and A20 unasserted.
fn record_marker(
    checks: &mut Checks,
    inventory: &ArchiveInventory<'_>,
) -> Result<(), ProfileError> {
    let outcome = match locate::marker(inventory)? {
        Marker::Matching => CheckOutcome::Pass,
        Marker::ContentMismatch => CheckOutcome::Fail(codes::MARKER_CONTENT),
        Marker::NotFirst => CheckOutcome::Fail(codes::MARKER_NOT_FIRST),
        Marker::Prefixed => CheckOutcome::Unresolved(RuleId::A19),
        Marker::Missing => CheckOutcome::Fail(codes::MARKER_MISSING),
    };
    checks.record(CheckId::MarkerEntry, outcome);
    Ok(())
}

/// Copy the observable header facts out of a parsed document.
fn observe_metadata(observations: &mut Observations, metadata: &Metadata) {
    observations.krx_verzioszam = Some(metadata.header.version.clone());
    observations.forrasrendszer_azonosito = Some(metadata.header.source_system);
    observations.kuldemeny_tipus = Some(metadata.header.consignment_kind);
    observations.attachment_count = Some(metadata.attachment_count());
}

/// Record every check that needs a parsed document.
fn record_document_checks(
    checks: &mut Checks,
    metadata: &Metadata,
    attachments: &[AttachmentResolution],
) {
    checks.record(
        CheckId::SchemaOptionalFields,
        if schema_optional_fields_complete(metadata) {
            CheckOutcome::Pass
        } else {
            CheckOutcome::Unresolved(RuleId::M11)
        },
    );
    checks.record(
        CheckId::HandlingInstructionsForm,
        if metadata
            .dispatches
            .iter()
            .any(|dispatch| dispatch.handling_instructions_unqualified)
        {
            CheckOutcome::Pass
        } else {
            CheckOutcome::NotApplicable
        },
    );
    checks.record(
        CheckId::AttachmentReferences,
        reference_outcome(attachments),
    );
    if let Some(agrees) = references::count_agrees(metadata) {
        checks.record(
            CheckId::AttachmentCount,
            if agrees {
                CheckOutcome::Pass
            } else {
                CheckOutcome::Fail(codes::COUNT_MISMATCH)
            },
        );
    }
    if !attachments.is_empty() {
        checks.record(
            CheckId::AttachmentUniqueness,
            if references::has_duplicate(attachments) {
                CheckOutcome::Fail(codes::REFERENCE_DUPLICATE)
            } else {
                CheckOutcome::Pass
            },
        );
        // M13: the unit and rounding of MERET are unresolved, so a declared
        // size is reported beside the observed one and never compared to it.
        checks.record(CheckId::DeclaredSize, CheckOutcome::Unresolved(RuleId::M13));
    }
}

/// Whether the elements M11 shows one official example omitting are present.
fn schema_optional_fields_complete(metadata: &Metadata) -> bool {
    metadata.header.test_present
        && metadata
            .attachments()
            .all(|attachment| attachment.description.is_some() && attachment.size_value.is_some())
}

/// Fail on an unresolvable reference, else report M14 for a prefix variant.
fn reference_outcome(attachments: &[AttachmentResolution]) -> CheckOutcome {
    if attachments.is_empty() {
        return CheckOutcome::NotApplicable;
    }
    if attachments
        .iter()
        .any(|attachment| attachment.resolution == ReferenceResolution::Missing)
    {
        return CheckOutcome::Fail(codes::REFERENCE_MISSING_ENTRY);
    }
    if attachments.iter().any(|attachment| {
        matches!(
            attachment.resolution,
            ReferenceResolution::PrefixVariant { .. }
        )
    }) {
        return CheckOutcome::Unresolved(RuleId::M14);
    }
    CheckOutcome::Pass
}
