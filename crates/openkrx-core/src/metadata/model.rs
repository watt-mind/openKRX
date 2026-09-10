//! Typed shape of a `KER_META_V0_9` document.
//!
//! The structs re-express rules M1 to M8 of `docs/profile.md` as code. No XSD
//! is vendored; each type carries the rule it encodes in its documentation.
//! Nothing here is a conformance statement: a parsed [`Metadata`] value says
//! only that the bytes matched this grammar.
//!
//! Values that the sources leave open are kept as they were written. `MERET`
//! (M13, unresolved) keeps its verbatim text alongside an optional numeric
//! reading, and `KULDEMENY_LETREHOZASANAK_IDEJE` keeps its `xs:dateTime`
//! lexical form verbatim: this crate has no clock and does not interpret time.

/// The `FORRASRENDSZER_AZONOSITO` enumeration (M4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SourceSystem {
    /// `NOVA`.
    Nova,
    /// `KIR3`.
    Kir3,
    /// `KER`.
    Ker,
    /// `POSTA`.
    Posta,
    /// `IMAP`.
    Imap,
}

impl SourceSystem {
    /// The token exactly as the schema spells it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Nova => "NOVA",
            Self::Kir3 => "KIR3",
            Self::Ker => "KER",
            Self::Posta => "POSTA",
            Self::Imap => "IMAP",
        }
    }

    /// Parse a token, byte-exactly; the enumeration fixes the spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        [Self::Nova, Self::Kir3, Self::Ker, Self::Posta, Self::Imap]
            .into_iter()
            .find(|value| value.as_str() == text)
    }
}

/// The `KULDEMENY_TIPUS` enumeration (M4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ConsignmentKind {
    /// `KULDEMENY`.
    Kuldemeny,
    /// `NYUGTA`.
    Nyugta,
    /// `EXPEDIALAS`.
    Expedialas,
    /// `TERTIVEVENY`.
    Tertiveveny,
    /// `HIBAJELZES`.
    Hibajelzes,
}

impl ConsignmentKind {
    /// The token exactly as the schema spells it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Kuldemeny => "KULDEMENY",
            Self::Nyugta => "NYUGTA",
            Self::Expedialas => "EXPEDIALAS",
            Self::Tertiveveny => "TERTIVEVENY",
            Self::Hibajelzes => "HIBAJELZES",
        }
    }

    /// Parse a token, byte-exactly; the enumeration fixes the spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        [
            Self::Kuldemeny,
            Self::Nyugta,
            Self::Expedialas,
            Self::Tertiveveny,
            Self::Hibajelzes,
        ]
        .into_iter()
        .find(|value| value.as_str() == text)
    }
}

/// `FEJRESZ`, the required header (M3).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Header {
    /// `KRX_VERZIOSZAM`, the only machine-readable profile version token.
    pub version: String,
    /// `FORRASRENDSZER_AZONOSITO` (M4).
    pub source_system: SourceSystem,
    /// `KULDEMENY_AZONOSITO`.
    pub consignment_id: String,
    /// `KULDEMENY_LETREHOZASANAK_IDEJE`, the `xs:dateTime` lexical form,
    /// retained verbatim and never interpreted.
    pub created_at_text: String,
    /// `KULDEMENY_TIPUS` (M4).
    pub consignment_kind: ConsignmentKind,
    /// `TESZT`, defaulting to `false` when the element is absent.
    pub test: bool,
    /// Whether `TESZT` was actually present.
    ///
    /// Its absence is unresolved rule M11, not an error: one official example
    /// omits an element the schema requires.
    pub test_present: bool,
    /// `VONALKOD`, optional.
    pub barcode: Option<String>,
    /// `KULDEMENY_HIVATKOZASI_AZONOSITO`, optional.
    pub reference_id: Option<String>,
    /// `HIBAKOD`, optional.
    pub error_code: Option<String>,
    /// `KULDEMENY_MEGJEGYZES`, optional.
    pub note: Option<String>,
}

/// One `MELLEKLET` attachment reference (M5, M6).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct AttachmentReference {
    /// `MELLEKLET_LEIRASA`.
    ///
    /// Schema-required, but one official example omits it, which is unresolved
    /// rule M11; absence is therefore recorded rather than rejected.
    pub description: Option<String>,
    /// `CSATOLMANY_SZAMA`, declared `xs:long`.
    pub number: i64,
    /// `FAJL_NEV`.
    pub file_name: String,
    /// `MERET` exactly as written.
    ///
    /// Its unit and rounding are unresolved rule M13, and one official example
    /// writes a non-numeric value (M11), so the text is authoritative here.
    pub size_text: String,
    /// `MERET` read as `xs:double`, when it is one.
    pub size_value: Option<f64>,
    /// `ELHELYEZKEDES`, the location inside the packed container.
    pub location: String,
    /// `MENNYISEG`, optional.
    pub quantity: Option<String>,
    /// `MENNYISEGI_EGYSEG`, optional.
    pub quantity_unit: Option<String>,
}

impl AttachmentReference {
    /// The declared path: `ELHELYEZKEDES`, then `/`, then `FAJL_NEV` (M10).
    ///
    /// Trailing separators on the location are collapsed so that `Payload/ID-1`
    /// and `Payload/ID-1/` produce the same path. Whether the result is
    /// archive-root-relative is unresolved rule M14; resolution against real
    /// entry names happens in `crate::profile`.
    #[must_use]
    pub fn declared_path(&self) -> String {
        let location = self.location.trim_end_matches('/');
        if location.is_empty() {
            return self.file_name.clone();
        }
        let mut path = String::with_capacity(location.len() + 1 + self.file_name.len());
        path.push_str(location);
        path.push('/');
        path.push_str(&self.file_name);
        path
    }
}

/// One `EXPEDIALAS` dispatch block (M7, M8).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Dispatch {
    /// `MELLEKLETEK_SZAMA`, the declared attachment count.
    ///
    /// The schema requires it beside the list, so the two can disagree; M7
    /// therefore makes both a check rather than an assumption.
    pub declared_attachment_count: Option<i64>,
    /// The `MELLEKLET` references actually listed under `MELLEKLETEK`.
    pub attachments: Vec<AttachmentReference>,
    /// Whether the `MELLEKLETEK` container element itself was present (M7).
    ///
    /// The references live inside it, so a dispatch that lists one always
    /// carries it. The flag is what the list alone cannot express: an empty
    /// container and no container at all are different documents, and a writer
    /// that always emitted the element would give the second the shape of the
    /// first — a change nobody asked for. The fact is therefore retained here
    /// rather than derived from [`attachments`](Dispatch::attachments) being
    /// empty.
    pub attachments_present: bool,
    /// Whether a namespace-unqualified `KEZELESI_UTASITASOK` was present (M8).
    pub handling_instructions_unqualified: bool,
}

impl Dispatch {
    /// The same block, declaring whether it carries a `MELLEKLETEK` container.
    ///
    /// The builder form of
    /// [`attachments_present`](Dispatch::attachments_present), for callers
    /// outside this crate: the type is `#[non_exhaustive]`, so nothing there
    /// can set the field on the value [`crate::draft::dispatch`] returned.
    #[must_use]
    pub fn with_attachment_container(mut self, present: bool) -> Self {
        self.attachments_present = present;
        self
    }
}

/// A parsed `KER_META_V0_9`-shaped document.
///
/// A value of this type is an observation about bytes, not a statement that the
/// enclosing archive is a conforming KRX package: `docs/profile.md` leaves
/// rules A19 to A22 and M11 to M15 open.
///
/// Every field is package content — identifiers, file names and free text. The
/// `Debug` representation prints it verbatim and must never be logged,
/// persisted or sent through telemetry; error codes and `Display` carry no
/// document content and are what diagnostics may report.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Metadata {
    /// `FEJRESZ` (M2, M3).
    pub header: Header,
    /// Whether `ERKEZTETES` was present (M2). Its content is not interpreted.
    pub receipt_present: bool,
    /// Whether `BONTASOK` was present (M2). Its content is not interpreted.
    pub openings_present: bool,
    /// Whether `TERTIVEVENY` was present (M2). Its content is not interpreted.
    pub return_receipt_present: bool,
    /// `EXPEDIALAS` blocks under `EXPEDIALASOK` (M2, M7).
    pub dispatches: Vec<Dispatch>,
    /// Elements the grammar does not define, counted rather than rejected.
    ///
    /// Rule A9 records that a container may carry further descriptive
    /// information, so an unknown element is reported, not treated as invalid.
    pub unknown_elements: u32,
}

impl Metadata {
    /// Every attachment reference, in document order, across all dispatches.
    pub fn attachments(&self) -> impl Iterator<Item = &AttachmentReference> {
        self.dispatches
            .iter()
            .flat_map(|dispatch| dispatch.attachments.iter())
    }

    /// Total number of attachment references listed in the document.
    #[must_use]
    pub fn attachment_count(&self) -> usize {
        self.dispatches
            .iter()
            .map(|dispatch| dispatch.attachments.len())
            .sum()
    }
}
