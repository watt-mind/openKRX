//! What a caller asks to change about a package.
//!
//! [`Edits`] is a pure value: header fields to set, attachment bytes to add or
//! replace, and attachment numbers to remove. Nothing here reads a clock,
//! opens a path or consults the environment, and nothing here decides what the
//! result looks like — [`crate::repack::plan`] does that, and refuses an edit
//! the package cannot carry.
//!
//! Every value in this module is package content. The `Debug` representation
//! prints it verbatim and must never be logged, persisted or sent through
//! telemetry.

use crate::metadata::{ConsignmentKind, SourceSystem};

/// One `FEJRESZ` field an edit can set.
///
/// The names are the ones a caller writes, so a report can say which fields an
/// edit touched without repeating any value that was written into them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum HeaderField {
    /// `KRX_VERZIOSZAM`.
    Version,
    /// `FORRASRENDSZER_AZONOSITO` (M4).
    SourceSystem,
    /// `KULDEMENY_AZONOSITO`.
    ConsignmentId,
    /// `KULDEMENY_LETREHOZASANAK_IDEJE`.
    CreatedAt,
    /// `KULDEMENY_TIPUS` (M4).
    ConsignmentKind,
    /// `TESZT`.
    Test,
    /// `VONALKOD`.
    Barcode,
    /// `KULDEMENY_HIVATKOZASI_AZONOSITO`.
    ReferenceId,
    /// `HIBAKOD`.
    ErrorCode,
    /// `KULDEMENY_MEGJEGYZES`.
    Note,
}

impl HeaderField {
    /// The field's stable name, as a caller spells it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Version => "version",
            Self::SourceSystem => "source_system",
            Self::ConsignmentId => "consignment_id",
            Self::CreatedAt => "created_at",
            Self::ConsignmentKind => "consignment_kind",
            Self::Test => "test",
            Self::Barcode => "barcode",
            Self::ReferenceId => "reference_id",
            Self::ErrorCode => "error_code",
            Self::Note => "note",
        }
    }
}

/// What to do with one optional header element.
///
/// An optional element has three states rather than two, and collapsing them
/// would make one of them unreachable: leaving `VONALKOD` alone, giving it a
/// value, and removing it are three different edits.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum OptionalEdit {
    /// Leave the element exactly as the package has it.
    #[default]
    Keep,
    /// Write this value, adding the element when it was absent.
    Set(String),
    /// Remove the element from the document.
    Clear,
}

impl OptionalEdit {
    /// Whether this edit changes anything.
    #[must_use]
    pub const fn is_keep(&self) -> bool {
        matches!(self, Self::Keep)
    }

    /// The value the element takes, once the edit is applied to `current`.
    fn resolve(&self, current: Option<String>) -> Option<String> {
        match self {
            Self::Keep => current,
            Self::Set(value) => Some(value.clone()),
            Self::Clear => None,
        }
    }
}

/// The `FEJRESZ` fields an edit sets, each one optional.
///
/// A field left `None` — or [`OptionalEdit::Keep`] — is the package's own
/// value, carried through untouched. Nothing is defaulted: repacking never
/// invents a value a caller did not write.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HeaderEdits {
    /// `KRX_VERZIOSZAM`.
    pub version: Option<String>,
    /// `FORRASRENDSZER_AZONOSITO` (M4).
    pub source_system: Option<SourceSystem>,
    /// `KULDEMENY_AZONOSITO`.
    pub consignment_id: Option<String>,
    /// `KULDEMENY_LETREHOZASANAK_IDEJE`, written verbatim and never
    /// interpreted: this crate has no clock.
    pub created_at_text: Option<String>,
    /// `KULDEMENY_TIPUS` (M4).
    pub consignment_kind: Option<ConsignmentKind>,
    /// `TESZT`. Setting it writes the element, so a document that omitted it —
    /// unresolved rule M11 — gains it.
    pub test: Option<bool>,
    /// `VONALKOD`.
    pub barcode: OptionalEdit,
    /// `KULDEMENY_HIVATKOZASI_AZONOSITO`.
    pub reference_id: OptionalEdit,
    /// `HIBAKOD`.
    pub error_code: OptionalEdit,
    /// `KULDEMENY_MEGJEGYZES`.
    pub note: OptionalEdit,
}

impl HeaderEdits {
    /// The fields this edit sets, in a fixed order.
    #[must_use]
    pub fn fields(&self) -> Vec<HeaderField> {
        let mut fields = Vec::new();
        let mut push = |set: bool, field: HeaderField| {
            if set {
                fields.push(field);
            }
        };
        push(self.version.is_some(), HeaderField::Version);
        push(self.source_system.is_some(), HeaderField::SourceSystem);
        push(self.consignment_id.is_some(), HeaderField::ConsignmentId);
        push(self.created_at_text.is_some(), HeaderField::CreatedAt);
        push(
            self.consignment_kind.is_some(),
            HeaderField::ConsignmentKind,
        );
        push(self.test.is_some(), HeaderField::Test);
        push(!self.barcode.is_keep(), HeaderField::Barcode);
        push(!self.reference_id.is_keep(), HeaderField::ReferenceId);
        push(!self.error_code.is_keep(), HeaderField::ErrorCode);
        push(!self.note.is_keep(), HeaderField::Note);
        fields
    }

    /// Apply the edit to `header`, leaving every field it does not set.
    pub(crate) fn apply(&self, header: &mut crate::metadata::Header) {
        if let Some(version) = &self.version {
            header.version = version.clone();
        }
        if let Some(source_system) = self.source_system {
            header.source_system = source_system;
        }
        if let Some(consignment_id) = &self.consignment_id {
            header.consignment_id = consignment_id.clone();
        }
        if let Some(created_at) = &self.created_at_text {
            header.created_at_text = created_at.clone();
        }
        if let Some(kind) = self.consignment_kind {
            header.consignment_kind = kind;
        }
        if let Some(test) = self.test {
            header.test = test;
            // The writer omits `TESZT` when it was absent (M11); a caller who
            // set the value asked for the element, so it is written.
            header.test_present = true;
        }
        header.barcode = self.barcode.resolve(header.barcode.take());
        header.reference_id = self.reference_id.resolve(header.reference_id.take());
        header.error_code = self.error_code.resolve(header.error_code.take());
        header.note = self.note.resolve(header.note.take());
    }
}

/// One attachment to add, with the bytes it carries.
///
/// The file name is one path component: the writer decides where the file
/// goes, exactly as it does for a package built from nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentAddition {
    /// `FAJL_NEV`: the file name, one component, no separator.
    pub file_name: String,
    /// The file's bytes, preserved exactly.
    pub bytes: Vec<u8>,
    /// `MELLEKLET_LEIRASA`, the human description. Omitting it is what makes
    /// the `schema_optional_fields` check cite M11.
    pub description: Option<String>,
}

impl AttachmentAddition {
    /// One attachment with a file name and its bytes, and nothing optional.
    #[must_use]
    pub fn new(file_name: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            file_name: file_name.into(),
            bytes: bytes.into(),
            description: None,
        }
    }

    /// The same, with `MELLEKLET_LEIRASA` supplied.
    #[must_use]
    pub fn described(
        file_name: impl Into<String>,
        bytes: impl Into<Vec<u8>>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            description: Some(description.into()),
            ..Self::new(file_name, bytes)
        }
    }
}

/// One attachment's bytes to replace, named by its number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentReplacement {
    /// The attachment's `CSATOLMANY_SZAMA` in the package being edited,
    /// counted from 1.
    pub number: u32,
    /// The bytes that take its place, preserved exactly.
    pub bytes: Vec<u8>,
}

/// Everything one repacking run changes.
///
/// An [`Edits`] value carrying nothing is the identity: repacking a package
/// this crate wrote with it produces that package's bytes again.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Edits {
    /// The `FEJRESZ` fields to set.
    pub header: HeaderEdits,
    /// Attachment numbers to remove, each counted from 1.
    pub remove: Vec<u32>,
    /// Attachment bytes to replace, each named by its number.
    pub replace: Vec<AttachmentReplacement>,
    /// Attachments to add, in the order they are appended to the package.
    pub add: Vec<AttachmentAddition>,
}

impl Edits {
    /// Whether this value asks for no change at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.header.fields().is_empty()
            && self.remove.is_empty()
            && self.replace.is_empty()
            && self.add.is_empty()
    }
}
