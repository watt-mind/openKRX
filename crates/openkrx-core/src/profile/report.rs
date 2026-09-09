//! The structural check inventory and its report.
//!
//! Every check the profile layer performs is named once, in a fixed order, and
//! reported with one of four outcomes. There is deliberately no `valid`,
//! `conforming`, `is_krx` or boolean verdict field anywhere in this module:
//! `docs/profile.md` leaves rules A19 to A22 and M11 to M15 open, so no
//! conformance claim is available to make.

use crate::archive::ArchiveEntry;
use crate::metadata::{ConsignmentKind, Metadata, SourceSystem};

/// An unresolved rule of `docs/profile.md`, cited where a check cannot decide.
///
/// The enum is `#[non_exhaustive]`: a rule leaving the unresolved list removes
/// nothing from the API, but a newly discovered ambiguity may add a variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RuleId {
    /// A19: directory nesting and the location of the `mimetype` entry.
    A19,
    /// A20: the marker entry's compression method and byte-exactness.
    A20,
    /// A21: entry-name character encoding and case rules.
    A21,
    /// A22: payload subdirectory naming.
    A22,
    /// M11: an official example that does not validate against the schema.
    M11,
    /// M12: metadata file-name casing.
    M12,
    /// M13: the unit and rounding of the declared attachment size.
    M13,
    /// M14: how a declared attachment location maps onto real entry names.
    M14,
    /// M15: which metadata fields a receiving service actually requires.
    M15,
}

impl RuleId {
    /// The rule's identifier as `docs/profile.md` writes it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::A19 => "A19",
            Self::A20 => "A20",
            Self::A21 => "A21",
            Self::A22 => "A22",
            Self::M11 => "M11",
            Self::M12 => "M12",
            Self::M13 => "M13",
            Self::M14 => "M14",
            Self::M15 => "M15",
        }
    }
}

/// One structural check, named once and reported once.
///
/// The order of the variants is the order [`StructureReport::checks`] uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CheckId {
    /// Exactly one `Metalayer/KULDEMENY_META.xml` candidate exists (A4).
    MetadataLocation,
    /// That candidate is spelled canonically (M12).
    MetadataFileName,
    /// The path before `Metalayer/` is the canonical root prefix (A19).
    RootPrefix,
    /// A first entry named `mimetype` holds `application/OCD+ZIP` (A2).
    MarkerEntry,
    /// The metadata document parses against the M1 to M8 grammar.
    MetadataParse,
    /// Schema-required elements one official example omits are present (M11).
    SchemaOptionalFields,
    /// `KEZELESI_UTASITASOK` appears namespace-unqualified (M8).
    HandlingInstructionsForm,
    /// Every declared attachment resolves to an archive entry (M5, M10).
    AttachmentReferences,
    /// `MELLEKLETEK_SZAMA` agrees with the listed references (M7).
    AttachmentCount,
    /// No attachment number or declared path repeats (A6).
    AttachmentUniqueness,
    /// Declared sizes can be compared with observed ones (M13).
    DeclaredSize,
}

impl CheckId {
    /// Every check, in the order the report lists them.
    pub const ORDER: [Self; 11] = [
        Self::MetadataLocation,
        Self::MetadataFileName,
        Self::RootPrefix,
        Self::MarkerEntry,
        Self::MetadataParse,
        Self::SchemaOptionalFields,
        Self::HandlingInstructionsForm,
        Self::AttachmentReferences,
        Self::AttachmentCount,
        Self::AttachmentUniqueness,
        Self::DeclaredSize,
    ];

    /// A stable snake-case identifier, suitable for machine-readable output.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MetadataLocation => "metadata_location",
            Self::MetadataFileName => "metadata_file_name",
            Self::RootPrefix => "root_prefix",
            Self::MarkerEntry => "marker_entry",
            Self::MetadataParse => "metadata_parse",
            Self::SchemaOptionalFields => "schema_optional_fields",
            Self::HandlingInstructionsForm => "handling_instructions_form",
            Self::AttachmentReferences => "attachment_references",
            Self::AttachmentCount => "attachment_count",
            Self::AttachmentUniqueness => "attachment_uniqueness",
            Self::DeclaredSize => "declared_size",
        }
    }
}

/// What one check concluded.
///
/// `Fail` carries a stable dotted code; `Unresolved` carries the rule of
/// `docs/profile.md` that prevents a conclusion. An unresolved rule is never
/// reported as a failure, and never as a pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CheckOutcome {
    /// The check held.
    Pass,
    /// The check did not hold, with a stable dotted code.
    Fail(&'static str),
    /// No primary source settles the rule, so nothing can be concluded.
    Unresolved(RuleId),
    /// The check has nothing to apply to in this archive.
    NotApplicable,
}

/// One named check and its outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Check {
    /// Which check.
    pub id: CheckId,
    /// What it concluded.
    pub outcome: CheckOutcome,
}

/// A one-word reading of the whole check list.
///
/// `Consistent` means only that no check failed and none was left unresolved.
/// **It is not a conformance claim**: openKRX cannot make one while
/// `docs/profile.md` lists unresolved essential rules, and a package that is
/// internally consistent may still be rejected by a real service (M15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum StructureSummary {
    /// No check failed and none was left unresolved.
    Consistent,
    /// At least one check failed.
    Inconsistent,
    /// No check failed, but at least one could not be decided.
    Unresolved,
}

/// How a declared attachment path relates to the archive's entry names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReferenceResolution {
    /// An entry name equals the declared path byte-exactly.
    Resolved {
        /// Central-directory index of the entry.
        entry: u32,
    },
    /// Only a root-prefix variant of the declared path names an entry (M14).
    PrefixVariant {
        /// Central-directory index of the entry.
        entry: u32,
    },
    /// No entry name matches, with or without a plausible prefix.
    Missing,
}

/// One declared attachment and what the archive actually holds for it.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct AttachmentResolution {
    /// `CSATOLMANY_SZAMA`.
    pub number: i64,
    /// `ELHELYEZKEDES` joined to `FAJL_NEV` (M10).
    pub declared_path: String,
    /// How the path relates to the archive's entry names.
    pub resolution: ReferenceResolution,
    /// `MERET` exactly as written.
    pub declared_size_text: String,
    /// `MERET` read as `xs:double`, when it is one.
    pub declared_size_value: Option<f64>,
    /// Decoded size of the resolved entry, in bytes.
    ///
    /// The two sizes are reported side by side and never compared: the unit and
    /// rounding of `MERET` are unresolved rule M13.
    pub observed_size: Option<u64>,
}

/// What the archive was observed to contain, beyond the check outcomes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct Observations {
    /// Entry-name bytes preceding `Metalayer/`, such as `KRX/OCD/` (A19).
    pub root_prefix: Option<Vec<u8>>,
    /// The metadata entry's name exactly as the archive spells it (M12).
    pub metadata_entry_name: Option<Vec<u8>>,
    /// Central-directory index of the metadata entry.
    pub metadata_entry_index: Option<u32>,
    /// `KRX_VERZIOSZAM`, the only machine-readable profile version token.
    pub krx_verzioszam: Option<String>,
    /// `FORRASRENDSZER_AZONOSITO` (M4).
    pub forrasrendszer_azonosito: Option<SourceSystem>,
    /// `KULDEMENY_TIPUS` (M4).
    pub kuldemeny_tipus: Option<ConsignmentKind>,
    /// Number of `MELLEKLET` references the document listed.
    pub attachment_count: Option<usize>,
}

/// The result of [`crate::profile::check`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct StructureReport {
    checks: Vec<Check>,
    observations: Observations,
    attachments: Vec<AttachmentResolution>,
    metadata: Option<Metadata>,
}

impl StructureReport {
    /// Assemble a report; `checks` must already be in [`CheckId::ORDER`].
    pub(crate) const fn new(
        checks: Vec<Check>,
        observations: Observations,
        attachments: Vec<AttachmentResolution>,
        metadata: Option<Metadata>,
    ) -> Self {
        Self {
            checks,
            observations,
            attachments,
            metadata,
        }
    }

    /// Every check, in [`CheckId::ORDER`].
    #[must_use]
    pub fn checks(&self) -> &[Check] {
        &self.checks
    }

    /// The outcome of one check.
    #[must_use]
    pub fn outcome(&self, id: CheckId) -> CheckOutcome {
        self.checks
            .iter()
            .find(|check| check.id == id)
            .map_or(CheckOutcome::NotApplicable, |check| check.outcome)
    }

    /// What the archive and document were observed to contain.
    #[must_use]
    pub const fn observations(&self) -> &Observations {
        &self.observations
    }

    /// Every declared attachment and the entry it resolved to, in order.
    #[must_use]
    pub fn attachments(&self) -> &[AttachmentResolution] {
        &self.attachments
    }

    /// The parsed document, when parsing succeeded.
    #[must_use]
    pub const fn metadata(&self) -> Option<&Metadata> {
        self.metadata.as_ref()
    }

    /// A one-word reading of the check list; **not** a conformance claim.
    #[must_use]
    pub fn summary(&self) -> StructureSummary {
        if self
            .checks
            .iter()
            .any(|check| matches!(check.outcome, CheckOutcome::Fail(_)))
        {
            return StructureSummary::Inconsistent;
        }
        if self
            .checks
            .iter()
            .any(|check| matches!(check.outcome, CheckOutcome::Unresolved(_)))
        {
            return StructureSummary::Unresolved;
        }
        StructureSummary::Consistent
    }
}

/// Accumulates checks so the report can only be built in the fixed order.
#[derive(Default)]
pub(crate) struct Checks {
    outcomes: Vec<(CheckId, CheckOutcome)>,
}

impl Checks {
    /// Record the outcome of one check.
    pub(crate) fn record(&mut self, id: CheckId, outcome: CheckOutcome) {
        self.outcomes.push((id, outcome));
    }

    /// Emit every check in [`CheckId::ORDER`], defaulting to not applicable.
    pub(crate) fn finish(self) -> Vec<Check> {
        CheckId::ORDER
            .into_iter()
            .map(|id| Check {
                id,
                outcome: self
                    .outcomes
                    .iter()
                    .find(|(recorded, _)| *recorded == id)
                    .map_or(CheckOutcome::NotApplicable, |(_, outcome)| *outcome),
            })
            .collect()
    }
}

/// Observed decoded size of an entry, for side-by-side reporting only.
pub(crate) fn observed_size(entry: Option<&ArchiveEntry<'_>>) -> Option<u64> {
    entry.map(ArchiveEntry::decoded_size)
}
