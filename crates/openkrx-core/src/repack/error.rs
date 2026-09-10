//! Stable, content-free repacking error reporting.
//!
//! Every repacking failure exposes a dotted [`RepackError::code`] grouped by
//! category: `repack.unsupported.*` for an input this crate cannot re-emit
//! without changing or dropping part of it, and `repack.invalid.*` for an edit
//! that names something the package does not hold. The three layers repacking
//! composes report through their own codes unchanged — `archive.*` for
//! re-reading an entry, `metadata.*` for parsing the document and `create.*`
//! for writing the result — so a consumer buckets a repack diagnostic exactly
//! as it buckets every other one.
//!
//! As everywhere else, a diagnostic carries a stable code, an index, an
//! attachment number and numbers. It never carries a file name, a metadata
//! value, an edit a caller wrote or a byte of payload.

use core::fmt;

use crate::create::CreateError;
use crate::error::ArchiveError;
use crate::metadata::MetadataError;

/// Something the input package carries that the writer cannot re-emit.
///
/// Each variant is a fact about the *input*, never about the edits: repacking
/// refuses rather than writing a package that silently differs from the one it
/// was given in a way nobody asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnsupportedKind {
    /// No entry has the shape of a metadata document, or more than one does,
    /// so there is no single document to edit.
    MetadataMissing,
    /// The metadata document does not sit under the canonical `KRX/OCD/`
    /// prefix. Rule A19 leaves the prefix open and the writer emits one of the
    /// three layouts, so repacking would move every entry of the package.
    RootPrefix,
    /// The metadata document's last two segments are not spelled
    /// `Metalayer/KULDEMENY_META.xml` byte-exactly. Rule M12 leaves the casing
    /// open, so re-emitting it would rename the entry.
    MetadataName,
    /// The archive's first entry is not `KRX/OCD/mimetype` holding
    /// `application/OCD+ZIP` (A2, A19).
    Marker,
    /// An entry is neither the marker, the metadata document, nor an
    /// attachment at `KRX/OCD/Payload/ID-<n>/<file>` (A5, A22). The writer
    /// emits those three and nothing else, so such an entry would be dropped —
    /// `signatures.xml` (A7) and a service-specific document (A11–A16) reach
    /// this variant.
    ExtraEntry,
    /// The document counted elements the grammar does not define (A9). The
    /// reader counts them and keeps none, so the writer cannot reproduce them.
    UnknownElements,
    /// The document carries a block whose content the reader records only the
    /// presence of: `ERKEZTETES`, `BONTASOK`, `TERTIVEVENY` (M2) or an
    /// unqualified `KEZELESI_UTASITASOK` (M8). Re-emitting it would write the
    /// element back empty.
    OpaqueBlock,
    /// The document carries more than one `EXPEDIALAS` block, which leaves no
    /// single place for the derived references (M7).
    DispatchCount,
    /// A `MELLEKLET` reference, or `MELLEKLETEK_SZAMA`, is not what this
    /// writer derives for the attachments the document declares: a different
    /// number, location, file name, `MERET` value or count. Re-emitting the
    /// document would rewrite it, so the package is refused instead.
    AttachmentReference,
    /// The payload entries are not exactly the ones the references name, in
    /// the order the layout numbers them.
    AttachmentEntry,
}

impl UnsupportedKind {
    /// Return the stable dotted identifier for this variant.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::MetadataMissing => "repack.unsupported.metadata_missing",
            Self::RootPrefix => "repack.unsupported.root_prefix",
            Self::MetadataName => "repack.unsupported.metadata_name",
            Self::Marker => "repack.unsupported.marker",
            Self::ExtraEntry => "repack.unsupported.extra_entry",
            Self::UnknownElements => "repack.unsupported.unknown_elements",
            Self::OpaqueBlock => "repack.unsupported.opaque_block",
            Self::DispatchCount => "repack.unsupported.dispatch_count",
            Self::AttachmentReference => "repack.unsupported.attachment_reference",
            Self::AttachmentEntry => "repack.unsupported.attachment_entry",
        }
    }
}

/// An edit that does not name something the package holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum InvalidKind {
    /// An edit names an attachment number the package does not carry.
    /// Numbers are the document's own `CSATOLMANY_SZAMA`, counted from 1.
    NoSuchAttachment,
    /// Two edits name the same attachment number, so what the caller wants
    /// done to it is not decided by the request.
    DuplicateTarget,
    /// [`crate::repack::apply`] was given an inventory other than the one
    /// [`crate::repack::plan`] read, so the bytes it would preserve are not
    /// the bytes the plan describes.
    InventoryMismatch,
}

impl InvalidKind {
    /// Return the stable dotted identifier for this variant.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::NoSuchAttachment => "repack.invalid.no_such_attachment",
            Self::DuplicateTarget => "repack.invalid.duplicate_target",
            Self::InventoryMismatch => "repack.invalid.inventory_mismatch",
        }
    }
}

/// Why a package could not be repacked.
///
/// A failure refuses the whole package. Nothing is written partially and
/// nothing is dropped: a caller handed a quietly reduced package would send
/// one that is not the package they edited.
///
/// The enum is `#[non_exhaustive]`: match on [`RepackError::code`] for stable
/// behaviour across versions, and treat an unknown code as a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RepackError {
    /// The input carries something the writer cannot re-emit.
    Unsupported {
        /// Which part of the input.
        kind: UnsupportedKind,
        /// Central-directory index of the entry it concerns, when it concerns
        /// one.
        entry: Option<u32>,
        /// The attachment number it concerns, when it concerns one.
        number: Option<u32>,
    },
    /// An edit names something the package does not hold.
    Invalid {
        /// Which kind of edit.
        kind: InvalidKind,
        /// The attachment number it concerns, when it concerns one.
        number: Option<u32>,
    },
    /// Re-reading an entry of the input failed.
    Read(ArchiveError),
    /// The metadata document could not be parsed.
    Document(MetadataError),
    /// The writer refused the package the edits describe.
    Write(CreateError),
}

impl RepackError {
    /// Return the stable dotted identifier for this error.
    ///
    /// ```
    /// use openkrx_core::repack::{RepackError, UnsupportedKind};
    ///
    /// let error = RepackError::Unsupported {
    ///     kind: UnsupportedKind::RootPrefix,
    ///     entry: Some(1),
    ///     number: None,
    /// };
    /// assert_eq!(error.code(), "repack.unsupported.root_prefix");
    /// ```
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match *self {
            Self::Unsupported { kind, .. } => kind.code(),
            Self::Invalid { kind, .. } => kind.code(),
            Self::Read(error) => error.code(),
            Self::Document(error) => error.code(),
            Self::Write(error) => error.code(),
        }
    }

    /// Central-directory index of the entry the error concerns, if any.
    #[must_use]
    pub const fn entry_index(&self) -> Option<u32> {
        match *self {
            Self::Unsupported { entry, .. } => entry,
            Self::Read(error) => error.entry_index(),
            _ => None,
        }
    }

    /// The attachment number the error concerns, if any.
    ///
    /// It is the document's own `CSATOLMANY_SZAMA`, counted from 1, not a
    /// central-directory index and not a position in the edits.
    #[must_use]
    pub const fn attachment_number(&self) -> Option<u32> {
        match *self {
            Self::Unsupported { number, .. } | Self::Invalid { number, .. } => number,
            _ => None,
        }
    }
}

impl fmt::Display for RepackError {
    /// Print the code and the positions only.
    ///
    /// The edits, the file names and the metadata values a package carries are
    /// deliberately excluded, so a repacking diagnostic stays as safe to log as
    /// a reading one.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Read(error) => return error.fmt(f),
            Self::Document(error) => return error.fmt(f),
            Self::Write(error) => return error.fmt(f),
            _ => f.write_str(self.code())?,
        }
        if let Some(entry) = self.entry_index() {
            write!(f, " at entry {entry}")?;
        }
        if let Some(number) = self.attachment_number() {
            write!(f, " at attachment number {number}")?;
        }
        Ok(())
    }
}

impl std::error::Error for RepackError {}

impl From<ArchiveError> for RepackError {
    fn from(error: ArchiveError) -> Self {
        Self::Read(error)
    }
}

impl From<MetadataError> for RepackError {
    fn from(error: MetadataError) -> Self {
        Self::Document(error)
    }
}

impl From<CreateError> for RepackError {
    fn from(error: CreateError) -> Self {
        Self::Write(error)
    }
}
