//! Stable, content-free creation error reporting.
//!
//! Every creation failure exposes a dotted [`CreateError::code`] grouped by
//! category: `create.invalid.*` for a request that contradicts itself or
//! carries something the documented layout cannot express, `create.over_limit.*`
//! for a documented ceiling the output would exceed, and `create.unsafe_name.*`
//! for a name openKRX refuses to write because its own reader or its own
//! extraction planner would refuse to read it back.
//!
//! As in every other layer, a diagnostic carries a stable code, the index of
//! the attachment it concerns and numbers. It never carries a file name, a
//! metadata value or a byte of payload.

use core::fmt;

/// A creation request that cannot be turned into the documented layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum InvalidKind {
    /// A caller-supplied `MELLEKLET` reference, or `MELLEKLETEK_SZAMA`,
    /// disagrees with the attachments the request actually carries.
    ReferenceMismatch,
    /// The attachments and the metadata cannot be placed in one another:
    /// attachments were supplied with no `EXPEDIALAS` block to list them in,
    /// or the document carries more than one block, which leaves no single
    /// place for the derived references.
    DispatchCount,
    /// A text value holds a character XML 1.0 cannot carry: a C0 control other
    /// than tab and line feed, a carriage return, which a parser would rewrite,
    /// or a non-character code point.
    Text,
    /// A text value starts or ends with whitespace. The reader trims leaf text,
    /// so writing it would produce a document that parses back to a different
    /// value.
    UntrimmedText,
    /// The document counted elements the grammar does not define. The writer
    /// emits the grammar and nothing else, so it cannot reproduce them and
    /// refuses rather than dropping content silently.
    UnknownElements,
    /// A calendar timestamp falls outside what an MS-DOS date and time field
    /// can express: 1980-01-01 00:00:00 to 2107-12-31 23:59:58, even seconds.
    Timestamp,
}

impl InvalidKind {
    /// Return the stable dotted identifier for this variant.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ReferenceMismatch => "create.invalid.reference_mismatch",
            Self::DispatchCount => "create.invalid.dispatch_count",
            Self::Text => "create.invalid.text",
            Self::UntrimmedText => "create.invalid.untrimmed_text",
            Self::UnknownElements => "create.invalid.unknown_elements",
            Self::Timestamp => "create.invalid.timestamp",
        }
    }
}

/// Which ceiling the output would exceed.
///
/// The values come from [`crate::Limits`], the same ceilings the reader
/// enforces, so a package this crate writes is one this crate can read back
/// under the same configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CreateLimitKind {
    /// `max_archive_bytes`, or the 4 GiB ceiling the format itself imposes on
    /// a writer that never emits ZIP64, whichever is smaller.
    ArchiveBytes,
    /// `max_entries`, counting the marker and the metadata document.
    Entries,
    /// `max_name_bytes`, applied to a written entry name.
    NameBytes,
    /// `max_entry_decoded_bytes`, applied to the bytes of one entry.
    EntryBytes,
    /// `max_total_decoded_bytes`, applied to every entry together.
    TotalBytes,
}

impl CreateLimitKind {
    /// Return the stable dotted identifier for this variant.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ArchiveBytes => "create.over_limit.archive_bytes",
            Self::Entries => "create.over_limit.entries",
            Self::NameBytes => "create.over_limit.name_bytes",
            Self::EntryBytes => "create.over_limit.entry_bytes",
            Self::TotalBytes => "create.over_limit.total_bytes",
        }
    }
}

/// An entry name this crate refuses to write.
///
/// The classes mirror the archive layer's name rules and the extraction
/// planner's component rules, so that what openKRX writes it can read back and
/// extract on all three target platforms. The name itself is never reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnsafeNameKind {
    /// A file name, or a component of one, is empty.
    Empty,
    /// A file name holds `/` or `\`. An attachment file name is one component:
    /// the writer places it, and never lets a caller build a path.
    Separator,
    /// A component is `.`, which names the directory it sits in.
    CurrentComponent,
    /// A component is `..`, which escapes the directory it sits in.
    ParentComponent,
    /// A component holds a C0 or C1 control character.
    ControlCharacter,
    /// A component ends with `.`, which several filesystems silently strip.
    TrailingDot,
    /// A component ends with a space, stripped the same way.
    TrailingSpace,
    /// A component starts with a space, stripped the same way.
    LeadingSpace,
    /// A component is a Windows reserved device name such as `CON` or `LPT1`,
    /// with or without an extension.
    ReservedDeviceName,
    /// A component holds `:`, which names an alternate data stream on NTFS.
    ReservedColon,
    /// A component holds one of `*`, `?`, `<`, `>`, `|` or `"`, none of which
    /// a Windows filesystem accepts in a name.
    ReservedCharacter,
}

impl UnsafeNameKind {
    /// Return the stable dotted identifier for this variant.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Empty => "create.unsafe_name.empty",
            Self::Separator => "create.unsafe_name.separator",
            Self::CurrentComponent => "create.unsafe_name.current_component",
            Self::ParentComponent => "create.unsafe_name.parent_component",
            Self::ControlCharacter => "create.unsafe_name.control_character",
            Self::TrailingDot => "create.unsafe_name.trailing_dot",
            Self::TrailingSpace => "create.unsafe_name.trailing_space",
            Self::LeadingSpace => "create.unsafe_name.leading_space",
            Self::ReservedDeviceName => "create.unsafe_name.reserved_device_name",
            Self::ReservedColon => "create.unsafe_name.colon",
            Self::ReservedCharacter => "create.unsafe_name.reserved_character",
        }
    }
}

/// Why a [`crate::create::PackageSpec`] could not be written.
///
/// A failure refuses the whole package. Nothing is written partially, renamed
/// or dropped: a caller handed a quietly reduced package would send one that is
/// not the package it asked for.
///
/// The enum is `#[non_exhaustive]`: match on [`CreateError::code`] for stable
/// behaviour across versions, and treat an unknown code as a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CreateError {
    /// The request contradicts itself or carries something the documented
    /// layout cannot express.
    Invalid {
        /// Which contradiction.
        kind: InvalidKind,
        /// Index of the attachment it concerns, when it concerns one.
        index: Option<u32>,
    },
    /// The output would exceed a documented ceiling.
    OverLimit {
        /// Which ceiling.
        limit: CreateLimitKind,
        /// The configured ceiling.
        limit_value: u64,
        /// The value that exceeded it, when meaningful.
        observed: Option<u64>,
        /// Index of the attachment it concerns, when it concerns one.
        index: Option<u32>,
    },
    /// An entry name this crate refuses to write.
    UnsafeName {
        /// Which unsafe shape. The name itself is never reported.
        kind: UnsafeNameKind,
        /// Index of the attachment it concerns, when it concerns one.
        index: Option<u32>,
    },
}

impl CreateError {
    /// Return the stable dotted identifier for this error.
    ///
    /// ```
    /// use openkrx_core::create::{CreateError, InvalidKind};
    ///
    /// let error = CreateError::Invalid {
    ///     kind: InvalidKind::ReferenceMismatch,
    ///     index: Some(0),
    /// };
    /// assert_eq!(error.code(), "create.invalid.reference_mismatch");
    /// ```
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match *self {
            Self::Invalid { kind, .. } => kind.code(),
            Self::OverLimit { limit, .. } => limit.code(),
            Self::UnsafeName { kind, .. } => kind.code(),
        }
    }

    /// Index of the attachment the error concerns, if any.
    ///
    /// This is a position in [`crate::create::PackageSpec::attachments`], not a
    /// central-directory index: nothing has been written when a request is
    /// refused.
    #[must_use]
    pub const fn attachment_index(&self) -> Option<u32> {
        match *self {
            Self::Invalid { index, .. }
            | Self::OverLimit { index, .. }
            | Self::UnsafeName { index, .. } => index,
        }
    }
}

impl fmt::Display for CreateError {
    /// Print the code, the numeric limit values and the attachment index only.
    ///
    /// File names, metadata values and payload bytes are deliberately excluded,
    /// so a creation diagnostic stays as safe to log as a reading one.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())?;
        if let Self::OverLimit {
            limit_value,
            observed,
            ..
        } = *self
        {
            write!(f, " (limit {limit_value}")?;
            if let Some(observed) = observed {
                write!(f, ", observed {observed}")?;
            }
            f.write_str(")")?;
        }
        if let Some(index) = self.attachment_index() {
            write!(f, " at attachment {index}")?;
        }
        Ok(())
    }
}

impl std::error::Error for CreateError {}
