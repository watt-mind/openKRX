//! Stable, content-free archive error reporting.
//!
//! Every error exposes a dotted [`ArchiveError::code`] grouped by category:
//! `archive.truncated.*`, `archive.malformed.*`, `archive.ambiguous.*`,
//! `archive.unsupported.*`, `archive.over_limit.*` and `archive.unsafe_name.*`.
//! Unsupported input, malformed input and resource exhaustion stay
//! distinguishable, and no diagnostic ever carries an entry name or entry
//! content: `Display` prints the code, the central-directory index of the
//! offending entry, and numeric limit values only.

use core::fmt;

/// Structure boundary at which the archive image ended unexpectedly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Structure {
    /// Central-directory record header or its variable-length fields.
    CentralDirectory,
    /// Local file header or its variable-length fields.
    LocalHeader,
    /// Compressed entry data.
    EntryData,
    /// Data descriptor following an entry written with general-purpose bit 3.
    DataDescriptor,
}

impl Structure {
    /// Return the stable dotted identifier for this variant.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::CentralDirectory => "archive.truncated.central_directory",
            Self::LocalHeader => "archive.truncated.local_header",
            Self::EntryData => "archive.truncated.entry_data",
            Self::DataDescriptor => "archive.truncated.data_descriptor",
        }
    }
}

/// Which documented limit an archive exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LimitKind {
    /// `max_archive_bytes`.
    ArchiveBytes,
    /// `max_entries`.
    Entries,
    /// `max_name_bytes`.
    NameBytes,
    /// `max_entry_decoded_bytes`.
    EntryDecodedBytes,
    /// `max_total_decoded_bytes`.
    TotalDecodedBytes,
    /// `max_compression_ratio`.
    CompressionRatio,
    /// `max_extra_field_bytes`.
    ExtraFieldBytes,
    /// `max_comment_bytes`.
    CommentBytes,
}

impl LimitKind {
    /// Return the stable dotted identifier for this variant.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ArchiveBytes => "archive.over_limit.archive_bytes",
            Self::Entries => "archive.over_limit.entries",
            Self::NameBytes => "archive.over_limit.name_bytes",
            Self::EntryDecodedBytes => "archive.over_limit.entry_decoded_bytes",
            Self::TotalDecodedBytes => "archive.over_limit.total_decoded_bytes",
            Self::CompressionRatio => "archive.over_limit.compression_ratio",
            Self::ExtraFieldBytes => "archive.over_limit.extra_field_bytes",
            Self::CommentBytes => "archive.over_limit.comment_bytes",
        }
    }
}

/// A structural contradiction in an otherwise complete archive image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MalformedKind {
    /// No end-of-central-directory record ends the image.
    EocdMissing,
    /// A record did not carry the signature its position requires.
    RecordSignature,
    /// The central directory holds a different record count than the EOCD.
    CentralDirectoryCount,
    /// The central directory does not occupy exactly the declared byte range.
    CentralDirectorySize,
    /// The central directory does not end where the EOCD record begins.
    CentralDirectoryPlacement,
    /// A local header contradicts its central-directory record.
    LocalHeaderMismatch,
    /// A data descriptor contradicts its central-directory record.
    DataDescriptorMismatch,
    /// Two claimed byte ranges overlap.
    OverlappingRanges,
    /// Bytes precede the first local header.
    PrefixBytes,
    /// Bytes follow the end-of-central-directory comment.
    TrailingBytes,
    /// Bytes belong to no local header, entry, central directory or EOCD.
    UnclaimedBytes,
    /// Decoded data did not match the declared CRC-32.
    CrcMismatch,
    /// Decoded size did not match the declared uncompressed size.
    DeclaredSizeMismatch,
    /// The deflate stream itself is invalid or ends early.
    DeflateStream,
}

impl MalformedKind {
    /// Return the stable dotted identifier for this variant.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::EocdMissing => "archive.malformed.eocd_missing",
            Self::RecordSignature => "archive.malformed.record_signature",
            Self::CentralDirectoryCount => "archive.malformed.central_directory_count",
            Self::CentralDirectorySize => "archive.malformed.central_directory_size",
            Self::CentralDirectoryPlacement => "archive.malformed.central_directory_placement",
            Self::LocalHeaderMismatch => "archive.malformed.local_header_mismatch",
            Self::DataDescriptorMismatch => "archive.malformed.data_descriptor_mismatch",
            Self::OverlappingRanges => "archive.malformed.overlapping_ranges",
            Self::PrefixBytes => "archive.malformed.prefix_bytes",
            Self::TrailingBytes => "archive.malformed.trailing_bytes",
            Self::UnclaimedBytes => "archive.malformed.unclaimed_bytes",
            Self::CrcMismatch => "archive.malformed.crc_mismatch",
            Self::DeclaredSizeMismatch => "archive.malformed.declared_size_mismatch",
            Self::DeflateStream => "archive.malformed.deflate_stream",
        }
    }
}

/// Input that admits more than one reading; rejected rather than resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AmbiguityKind {
    /// More than one end-of-central-directory record terminates the image.
    Eocd,
    /// Two entries carry byte-identical names.
    DuplicateName,
    /// Two entry names differ only by case; case rules are unresolved.
    CaseFoldedDuplicateName,
}

impl AmbiguityKind {
    /// Return the stable dotted identifier for this variant.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Eocd => "archive.ambiguous.eocd",
            Self::DuplicateName => "archive.ambiguous.duplicate_name",
            Self::CaseFoldedDuplicateName => "archive.ambiguous.case_folded_duplicate_name",
        }
    }
}

/// A well-formed ZIP feature this reader deliberately does not implement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnsupportedKind {
    /// A compression method other than stored (0) or deflate (8).
    Method,
    /// A ZIP64 record, marker value or extra field.
    Zip64,
    /// An encryption or strong-encryption general-purpose flag.
    Encryption,
    /// A multi-disk or split archive.
    MultiDisk,
    /// Compressed patched data (general-purpose bit 5).
    PatchedData,
}

impl UnsupportedKind {
    /// Return the stable dotted identifier for this variant.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Method => "archive.unsupported.method",
            Self::Zip64 => "archive.unsupported.zip64",
            Self::Encryption => "archive.unsupported.encryption",
            Self::MultiDisk => "archive.unsupported.multi_disk",
            Self::PatchedData => "archive.unsupported.patched_data",
        }
    }
}

/// A name shape that must never reach a filesystem layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnsafeNameKind {
    /// The name is empty.
    Empty,
    /// The name holds a NUL or another C0 control byte.
    ControlByte,
    /// The name holds a backslash, which is not a ZIP separator.
    Backslash,
    /// The name starts with `/`.
    AbsolutePath,
    /// The name holds a `..` path component.
    ParentComponent,
    /// The name starts with a Windows drive prefix such as `c:`.
    DrivePrefix,
    /// The name ends with a separator but declares content.
    DirectoryWithData,
}

impl UnsafeNameKind {
    /// Return the stable dotted identifier for this variant.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Empty => "archive.unsafe_name.empty",
            Self::ControlByte => "archive.unsafe_name.control_byte",
            Self::Backslash => "archive.unsafe_name.backslash",
            Self::AbsolutePath => "archive.unsafe_name.absolute_path",
            Self::ParentComponent => "archive.unsafe_name.parent_component",
            Self::DrivePrefix => "archive.unsafe_name.drive_prefix",
            Self::DirectoryWithData => "archive.unsafe_name.directory_with_data",
        }
    }
}

/// Why an archive could not be inventoried.
///
/// The enum is `#[non_exhaustive]`: match on [`ArchiveError::code`] for stable
/// behaviour across versions, and treat unknown codes as failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ArchiveError {
    /// A documented limit was reached.
    OverLimit {
        /// Which limit.
        limit: LimitKind,
        /// The configured ceiling.
        limit_value: u64,
        /// The value that reached or exceeded it, when meaningful.
        observed: Option<u64>,
        /// Central-directory index of the entry, when entry-scoped.
        entry: Option<u32>,
    },
    /// The image ended inside a structure.
    Truncated {
        /// Which structure.
        at: Structure,
        /// Central-directory index of the entry, when entry-scoped.
        entry: Option<u32>,
    },
    /// The image contradicts itself.
    Malformed {
        /// Which contradiction.
        kind: MalformedKind,
        /// Central-directory index of the entry, when entry-scoped.
        entry: Option<u32>,
    },
    /// The image admits more than one reading.
    Ambiguous {
        /// Which ambiguity.
        kind: AmbiguityKind,
        /// Central-directory index of the entry, when entry-scoped.
        entry: Option<u32>,
    },
    /// The image uses a feature this reader does not implement.
    Unsupported {
        /// Which feature.
        kind: UnsupportedKind,
        /// The offending numeric value, such as a method identifier.
        value: Option<u64>,
        /// Central-directory index of the entry, when entry-scoped.
        entry: Option<u32>,
    },
    /// An entry name is unsafe for any downstream filesystem use.
    UnsafeName {
        /// Which unsafe shape. The name bytes themselves are never reported.
        kind: UnsafeNameKind,
        /// Central-directory index of the entry.
        entry: u32,
    },
    /// The requested entry index does not exist in the inventory.
    NoSuchEntry {
        /// The requested index.
        entry: u32,
    },
}

impl ArchiveError {
    /// Return the stable dotted identifier for this error.
    ///
    /// ```
    /// use openkrx_core::{Limits, archive};
    ///
    /// let error = archive::inventory(b"not a zip", &Limits::DEFAULT).unwrap_err();
    /// assert_eq!(error.code(), "archive.malformed.eocd_missing");
    /// ```
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match *self {
            Self::OverLimit { limit, .. } => limit.code(),
            Self::Truncated { at, .. } => at.code(),
            Self::Malformed { kind, .. } => kind.code(),
            Self::Ambiguous { kind, .. } => kind.code(),
            Self::Unsupported { kind, .. } => kind.code(),
            Self::UnsafeName { kind, .. } => kind.code(),
            Self::NoSuchEntry { .. } => "archive.no_such_entry",
        }
    }

    /// Central-directory index of the entry the error concerns, if any.
    #[must_use]
    pub const fn entry_index(&self) -> Option<u32> {
        match *self {
            Self::OverLimit { entry, .. }
            | Self::Truncated { entry, .. }
            | Self::Malformed { entry, .. }
            | Self::Ambiguous { entry, .. }
            | Self::Unsupported { entry, .. } => entry,
            Self::UnsafeName { entry, .. } | Self::NoSuchEntry { entry } => Some(entry),
        }
    }
}

impl fmt::Display for ArchiveError {
    /// Print the code, entry index and numeric limit values only.
    ///
    /// Entry names, entry bytes and any other archive content are deliberately
    /// excluded so a diagnostic can be logged without leaking package content.
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
        if let Self::Unsupported {
            value: Some(value), ..
        } = *self
        {
            write!(f, " (value {value})")?;
        }
        if let Some(entry) = self.entry_index() {
            write!(f, " at entry {entry}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ArchiveError {}
