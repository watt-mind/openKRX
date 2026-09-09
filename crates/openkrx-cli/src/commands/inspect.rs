//! `inspect`: what the package declares about itself, and every check.
//!
//! This is the only command that prints a metadata value, and it prints it on
//! stdout only, because the user asked for it. Nothing here interprets a
//! value: `KULDEMENY_LETREHOZASANAK_IDEJE` keeps its lexical form because the
//! crate has no clock, and `MERET` keeps its verbatim text because rule M13
//! leaves its unit and rounding open. Declared and observed sizes are printed
//! beside one another and never compared.

use openkrx_core::archive::ArchiveInventory;
use openkrx_core::profile::{AttachmentResolution, CheckId, ReferenceResolution, StructureReport};
use serde::Serialize;

use super::{CheckView, OutcomeView, checks, hex_when_not_utf8, text};

/// What `inspect` reports.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InspectData {
    /// Facts about the archive's shape, independent of the document.
    pub observations: ObservationsView,
    /// The declared document, when one was located and parsed.
    pub metadata: Option<MetadataView>,
    /// Every structural check, in the documented order.
    pub checks: Vec<CheckView>,
}

/// What the archive was observed to contain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ObservationsView {
    /// Number of entries in the archive.
    pub entry_count: u32,
    /// Entry-name bytes preceding `Metalayer/`, when a document was located.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_prefix: Option<String>,
    /// Hexadecimal form of the prefix, only when it is not valid UTF-8.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_prefix_hex: Option<String>,
    /// The metadata entry's name exactly as the archive spells it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata_entry_name: Option<String>,
    /// Hexadecimal form of that name, only when it is not valid UTF-8.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata_entry_name_hex: Option<String>,
    /// Central-directory index of the metadata entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata_entry_index: Option<u32>,
    /// The outcome of the format-marker check (rule A2).
    pub marker: OutcomeView,
    /// The raw root-prefix bytes, for terminal-safe rendering.
    #[serde(skip)]
    pub root_prefix_bytes: Option<Vec<u8>>,
    /// The raw metadata-entry-name bytes, for terminal-safe rendering.
    #[serde(skip)]
    pub metadata_entry_name_bytes: Option<Vec<u8>>,
}

/// The declared fields of a parsed document, exactly as it wrote them.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MetadataView {
    /// `KRX_VERZIOSZAM`, the only machine-readable profile version token.
    pub version: String,
    /// `FORRASRENDSZER_AZONOSITO`.
    pub source_system: &'static str,
    /// `KULDEMENY_TIPUS`.
    pub consignment_type: &'static str,
    /// `KULDEMENY_AZONOSITO`.
    pub consignment_id: String,
    /// `KULDEMENY_HIVATKOZASI_AZONOSITO`, when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_id: Option<String>,
    /// `VONALKOD`, when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub barcode: Option<String>,
    /// `HIBAKOD`, when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    /// `KULDEMENY_LETREHOZASANAK_IDEJE`, verbatim and never interpreted.
    pub created_at: String,
    /// `TESZT`, defaulting to `false` when the element is absent.
    pub test: bool,
    /// Whether `TESZT` was actually present; its absence is rule M11.
    pub test_present: bool,
    /// `MELLEKLETEK_SZAMA`, summed across dispatch blocks, when declared.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declared_attachment_count: Option<i64>,
    /// Every `MELLEKLET` reference, in document order.
    pub attachments: Vec<AttachmentView>,
}

/// One declared attachment and the entry it resolved to.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AttachmentView {
    /// `CSATOLMANY_SZAMA`.
    pub number: i64,
    /// `ELHELYEZKEDES` joined to `FAJL_NEV`.
    pub declared_path: String,
    /// `resolved`, `prefix_variant` or `missing`.
    pub resolution: &'static str,
    /// Central-directory index of the entry the reference resolved to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_index: Option<u32>,
    /// `MERET` exactly as written; its unit is unresolved rule M13.
    pub declared_size_text: String,
    /// `MERET` read as a number, when it is a finite one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declared_size_value: Option<f64>,
    /// Decoded size of the resolved entry, reported beside the declared one
    /// and never compared to it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_size: Option<u64>,
}

/// Build the report from an inventory and its structural report.
#[must_use]
pub fn run(inventory: &ArchiveInventory<'_>, report: &StructureReport) -> InspectData {
    InspectData {
        observations: observations(inventory, report),
        metadata: report.metadata().map(|metadata| MetadataView {
            version: metadata.header.version.clone(),
            source_system: metadata.header.source_system.as_str(),
            consignment_type: metadata.header.consignment_kind.as_str(),
            consignment_id: metadata.header.consignment_id.clone(),
            reference_id: metadata.header.reference_id.clone(),
            barcode: metadata.header.barcode.clone(),
            error_code: metadata.header.error_code.clone(),
            created_at: metadata.header.created_at_text.clone(),
            test: metadata.header.test,
            test_present: metadata.header.test_present,
            declared_attachment_count: declared_count(metadata),
            attachments: report.attachments().iter().map(attachment).collect(),
        }),
        checks: checks(report),
    }
}

/// The archive-shaped facts, taken from the report's observations.
fn observations(inventory: &ArchiveInventory<'_>, report: &StructureReport) -> ObservationsView {
    let observed = report.observations();
    let prefix = observed.root_prefix.clone();
    let name = observed.metadata_entry_name.clone();
    ObservationsView {
        entry_count: u32::try_from(inventory.len()).unwrap_or(u32::MAX),
        root_prefix: prefix.as_deref().and_then(text),
        root_prefix_hex: prefix.as_deref().and_then(hex_when_not_utf8),
        metadata_entry_name: name.as_deref().and_then(text),
        metadata_entry_name_hex: name.as_deref().and_then(hex_when_not_utf8),
        metadata_entry_index: observed.metadata_entry_index,
        marker: OutcomeView::from(report.outcome(CheckId::MarkerEntry)),
        root_prefix_bytes: prefix,
        metadata_entry_name_bytes: name,
    }
}

/// `MELLEKLETEK_SZAMA`, summed over the dispatch blocks that declare one.
fn declared_count(metadata: &openkrx_core::metadata::Metadata) -> Option<i64> {
    let declared: Vec<i64> = metadata
        .dispatches
        .iter()
        .filter_map(|dispatch| dispatch.declared_attachment_count)
        .collect();
    if declared.is_empty() {
        return None;
    }
    Some(
        declared
            .iter()
            .fold(0_i64, |total, count| total.saturating_add(*count)),
    )
}

/// One resolution, with the numeric size reading dropped unless it is finite.
fn attachment(resolution: &AttachmentResolution) -> AttachmentView {
    let (word, entry) = match resolution.resolution {
        ReferenceResolution::Resolved { entry } => ("resolved", Some(entry)),
        ReferenceResolution::PrefixVariant { entry } => ("prefix_variant", Some(entry)),
        ReferenceResolution::Missing => ("missing", None),
        // The enum is `#[non_exhaustive]`; an unknown relation is not a match.
        _ => ("missing", None),
    };
    AttachmentView {
        number: resolution.number,
        declared_path: resolution.declared_path.clone(),
        resolution: word,
        entry_index: entry,
        declared_size_text: resolution.declared_size_text.clone(),
        // A non-finite `MERET` has no JSON number, and inventing one would be
        // a reading the sources do not support; the verbatim text stands.
        declared_size_value: resolution
            .declared_size_value
            .filter(|value| value.is_finite()),
        observed_size: resolution.observed_size,
    }
}
