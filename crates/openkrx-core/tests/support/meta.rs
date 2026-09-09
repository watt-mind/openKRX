//! Test-only synthetic metadata and KRX-shaped archive builders.
//!
//! Every document and archive the metadata and profile tests read is generated
//! here, in this repository, from freshly authored values. Nothing is copied
//! from an official sample: `docs/profile.md` records that no redistribution
//! licence exists for the posta.hu material, and `tests/fixtures/README.md`
//! records that the structural re-expression is synthetic.
//!
//! The builders deliberately allow documents the grammar rejects, so hostile
//! and merely unusual input can be constructed exactly.

use super::{Archive, Entry};

/// The metadata target namespace (`docs/profile.md` M1).
pub const NAMESPACE: &str = "http://xsd.orfk.hu/rzs/ker/kuldemeny";
/// The canonical metadata file name (A4, M12).
pub const METADATA_FILE: &str = "KULDEMENY_META.xml";
/// The format marker's content (A2).
pub const MARKER_CONTENT: &[u8] = b"application/OCD+ZIP";

/// One `MELLEKLET` reference, as text destined for the document.
#[derive(Clone)]
pub struct Attachment {
    /// `MELLEKLET_LEIRASA`, omitted when `None` (M11).
    pub description: Option<String>,
    /// `CSATOLMANY_SZAMA`.
    pub number: String,
    /// `FAJL_NEV`.
    pub file_name: String,
    /// `MERET`, written verbatim so a non-numeric value can be tested (M11).
    pub size: String,
    /// `ELHELYEZKEDES`.
    pub location: String,
}

impl Attachment {
    /// A complete, schema-shaped reference.
    #[must_use]
    pub fn new(number: i64, file_name: &str, location: &str) -> Self {
        Self {
            description: Some(format!("synthetic attachment {number}")),
            number: number.to_string(),
            file_name: file_name.to_owned(),
            size: "12.5".to_owned(),
            location: location.to_owned(),
        }
    }

    /// Serialise the reference with `prefix` on every qualified element.
    fn xml(&self, prefix: &str) -> String {
        let mut out = String::new();
        if let Some(description) = &self.description {
            out.push_str(&element(prefix, "MELLEKLET_LEIRASA", description));
        }
        out.push_str(&element(prefix, "CSATOLMANY_SZAMA", &self.number));
        out.push_str(&element(prefix, "FAJL_NEV", &self.file_name));
        out.push_str(&element(prefix, "MERET", &self.size));
        out.push_str(&element(prefix, "ELHELYEZKEDES", &self.location));
        format!("<{0}MELLEKLET>{1}</{0}MELLEKLET>", prefix, out)
    }
}

/// A `KULDEMENY` document, assembled from freshly authored values.
#[derive(Clone)]
pub struct Document {
    /// Namespace prefix, or the empty string for a default namespace.
    pub prefix: String,
    /// Namespace bound to that prefix; a mismatch tests M1.
    pub namespace: String,
    /// Whether to emit an XML declaration, as the official examples do (M9).
    pub declaration: bool,
    /// `KRX_VERZIOSZAM`.
    pub version: Option<String>,
    /// `FORRASRENDSZER_AZONOSITO` (M4).
    pub source_system: Option<String>,
    /// `KULDEMENY_AZONOSITO`.
    pub consignment_id: Option<String>,
    /// `KULDEMENY_LETREHOZASANAK_IDEJE`.
    pub created_at: Option<String>,
    /// `KULDEMENY_TIPUS` (M4).
    pub consignment_kind: Option<String>,
    /// `TESZT`, omitted when `None` (M11).
    pub test: Option<String>,
    /// `MELLEKLETEK_SZAMA`, omitted when `None` (M7).
    pub declared_count: Option<String>,
    /// The `MELLEKLET` references to list.
    pub attachments: Vec<Attachment>,
    /// Whether to emit an unqualified `KEZELESI_UTASITASOK` (M8).
    pub handling_instructions: bool,
    /// Raw text inserted directly inside `KULDEMENY`, after `FEJRESZ`.
    pub extra_body: String,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            prefix: "ns2".to_owned(),
            namespace: NAMESPACE.to_owned(),
            declaration: true,
            version: Some("v0.9".to_owned()),
            source_system: Some("KER".to_owned()),
            consignment_id: Some("SYN-0001".to_owned()),
            created_at: Some("2026-01-02T03:04:05.000+01:00".to_owned()),
            consignment_kind: Some("KULDEMENY".to_owned()),
            test: Some("false".to_owned()),
            declared_count: Some("1".to_owned()),
            attachments: vec![Attachment::new(1, "synthetic.pdf", "KRX/OCD/Payload/ID-1")],
            handling_instructions: true,
            extra_body: String::new(),
        }
    }
}

impl Document {
    /// A document with a header and no dispatch block at all.
    #[must_use]
    pub fn header_only() -> Self {
        Self {
            declared_count: None,
            attachments: Vec::new(),
            handling_instructions: false,
            ..Self::default()
        }
    }

    /// Serialise the document.
    #[must_use]
    pub fn xml(&self) -> String {
        let prefix = if self.prefix.is_empty() {
            String::new()
        } else {
            format!("{}:", self.prefix)
        };
        let binding = if self.prefix.is_empty() {
            format!(" xmlns=\"{}\"", self.namespace)
        } else {
            format!(" xmlns:{}=\"{}\"", self.prefix, self.namespace)
        };
        let mut out = String::new();
        if self.declaration {
            out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>");
        }
        out.push_str(&format!("<{prefix}KULDEMENY{binding}>"));
        out.push_str(&self.header_xml(&prefix));
        out.push_str(&self.extra_body);
        out.push_str(&self.dispatch_xml(&prefix));
        out.push_str(&format!("</{prefix}KULDEMENY>"));
        out
    }

    /// Serialise the document's bytes.
    #[must_use]
    pub fn bytes(&self) -> Vec<u8> {
        self.xml().into_bytes()
    }

    /// Serialise `FEJRESZ` (M3).
    fn header_xml(&self, prefix: &str) -> String {
        let mut out = String::new();
        for (name, value) in [
            ("KRX_VERZIOSZAM", &self.version),
            ("FORRASRENDSZER_AZONOSITO", &self.source_system),
            ("KULDEMENY_AZONOSITO", &self.consignment_id),
            ("KULDEMENY_LETREHOZASANAK_IDEJE", &self.created_at),
            ("KULDEMENY_TIPUS", &self.consignment_kind),
            ("TESZT", &self.test),
        ] {
            if let Some(value) = value {
                out.push_str(&element(prefix, name, value));
            }
        }
        format!("<{0}FEJRESZ>{1}</{0}FEJRESZ>", prefix, out)
    }

    /// Serialise `EXPEDIALASOK` (M2, M7, M8), or nothing when it is empty.
    fn dispatch_xml(&self, prefix: &str) -> String {
        if self.declared_count.is_none()
            && self.attachments.is_empty()
            && !self.handling_instructions
        {
            return String::new();
        }
        let mut inner = String::new();
        if let Some(count) = &self.declared_count {
            inner.push_str(&element(prefix, "MELLEKLETEK_SZAMA", count));
        }
        let listed: String = self
            .attachments
            .iter()
            .map(|attachment| attachment.xml(prefix))
            .collect();
        inner.push_str(&format!(
            "<{0}MELLEKLETEK>{1}</{0}MELLEKLETEK>",
            prefix, listed
        ));
        if self.handling_instructions {
            // M8: this one element is declared unqualified, so it carries no prefix.
            inner.push_str("<KEZELESI_UTASITASOK>synthetic instruction</KEZELESI_UTASITASOK>");
        }
        format!(
            "<{0}EXPEDIALASOK><{0}EXPEDIALAS>{1}</{0}EXPEDIALAS></{0}EXPEDIALASOK>",
            prefix, inner
        )
    }
}

/// One qualified leaf element carrying `value`.
fn element(prefix: &str, name: &str, value: &str) -> String {
    format!("<{prefix}{name}>{value}</{prefix}{name}>")
}

/// A KRX-shaped archive: marker, metadata document, then payload entries.
///
/// `root_prefix` is prepended to every entry name except the marker, so the
/// three layouts `docs/profile.md` A19 describes can each be built exactly.
#[must_use]
pub fn krx(root_prefix: &str, metadata_file: &str, document: &[u8], payloads: &[&str]) -> Vec<u8> {
    let mut entries = vec![
        Entry::stored(b"mimetype", MARKER_CONTENT),
        Entry::deflated(
            format!("{root_prefix}Metalayer/{metadata_file}").as_bytes(),
            document,
        ),
    ];
    for (index, name) in payloads.iter().enumerate() {
        entries.push(Entry::stored(
            name.as_bytes(),
            format!("synthetic payload {index}").as_bytes(),
        ));
    }
    Archive::of(entries).build()
}

/// The canonical single-attachment archive, with the layout `KRX-SPEC` shows.
#[must_use]
pub fn canonical_krx(document: &Document) -> Vec<u8> {
    krx(
        "KRX/OCD/",
        METADATA_FILE,
        &document.bytes(),
        &["KRX/OCD/Payload/ID-1/synthetic.pdf"],
    )
}
