//! Stable, content-free extraction-planning error reporting.
//!
//! Every planning failure exposes a dotted [`PlanError::code`] grouped by
//! category: `extract.unsupported.*` for an entry this crate will never write,
//! `extract.unsafe_path.*` for a destination component that must never reach a
//! filesystem, `extract.ambiguous.*` for two outputs that cannot both exist, and
//! `extract.over_limit.*` for a documented ceiling. As in the archive layer, no
//! diagnostic carries a name or any entry content: `Display` prints the code,
//! the central-directory index of the offending entry, and numbers.

use core::fmt;

/// An entry this crate refuses to plan output for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnsupportedEntryKind {
    /// The entry declares a symbolic link. A link is never created, and never
    /// materialised as its own target text either.
    Link,
    /// The entry declares a device node, socket, FIFO or another mode that is
    /// not a regular file, a directory or a link.
    SpecialFile,
    /// The entry name is not valid UTF-8, and rule A21 leaves the intended
    /// encoding unresolved, so no code page is guessed.
    NonUtf8Name,
}

impl UnsupportedEntryKind {
    /// Return the stable dotted identifier for this variant.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Link => "extract.unsupported.link",
            Self::SpecialFile => "extract.unsupported.special_file",
            Self::NonUtf8Name => "extract.unsupported.non_utf8_name",
        }
    }
}

/// A destination path component that must never reach a filesystem layer.
///
/// The archive layer already rejects several of these shapes as unsafe *names*.
/// They are checked again here, against the components the planner itself
/// produced, so the planner's own output is safe by construction rather than by
/// trusting an earlier layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnsafeComponentKind {
    /// A component is empty, as in `a//b` or a name ending in a separator that
    /// is not a directory marker.
    Empty,
    /// A component is `.`, which names the directory it sits in.
    CurrentComponent,
    /// A component is `..`, which escapes the directory it sits in.
    ParentComponent,
    /// A component holds a NUL, another C0 control character, or a C1 control
    /// character.
    ControlCharacter,
    /// A component ends with `.`, which several filesystems silently strip.
    TrailingDot,
    /// A component ends with a space, which several filesystems silently strip.
    TrailingSpace,
    /// A component starts with a space, which Windows Explorer and several
    /// tools silently strip, so two entries could become one file.
    LeadingSpace,
    /// A component is a Windows reserved device name such as `CON` or `LPT1`,
    /// with or without an extension.
    ReservedDeviceName,
    /// A component holds `:`, which names an alternate data stream on NTFS and
    /// a volume elsewhere.
    Colon,
    /// A component holds one of `*`, `?`, `<`, `>`, `|` or `"`, none of which
    /// a Windows filesystem accepts in a name.
    ReservedCharacter,
}

impl UnsafeComponentKind {
    /// Return the stable dotted identifier for this variant.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Empty => "extract.unsafe_path.empty_component",
            Self::CurrentComponent => "extract.unsafe_path.current_component",
            Self::ParentComponent => "extract.unsafe_path.parent_component",
            Self::ControlCharacter => "extract.unsafe_path.control_character",
            Self::TrailingDot => "extract.unsafe_path.trailing_dot",
            Self::TrailingSpace => "extract.unsafe_path.trailing_space",
            Self::LeadingSpace => "extract.unsafe_path.leading_space",
            Self::ReservedDeviceName => "extract.unsafe_path.reserved_device_name",
            Self::Colon => "extract.unsafe_path.colon",
            Self::ReservedCharacter => "extract.unsafe_path.reserved_character",
        }
    }
}

/// Two planned outputs that cannot both exist; refused rather than renamed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PlanAmbiguityKind {
    /// Two destination paths are equal after NFC normalisation and simple case
    /// folding, so a case-insensitive or normalising filesystem would see one
    /// path written twice.
    Collision,
    /// One entry's destination path is a directory prefix of another's, so one
    /// output would have to be both a file and a directory.
    FileDirectoryConflict,
}

impl PlanAmbiguityKind {
    /// Return the stable dotted identifier for this variant.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Collision => "extract.ambiguous.collision",
            Self::FileDirectoryConflict => "extract.ambiguous.file_directory_conflict",
        }
    }
}

/// Which documented extraction limit a plan exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExtractLimitKind {
    /// `max_files`.
    Files,
    /// `max_total_bytes`.
    TotalBytes,
    /// `max_path_bytes`.
    PathBytes,
    /// `max_component_bytes`.
    ComponentBytes,
    /// `max_depth`.
    Depth,
}

impl ExtractLimitKind {
    /// Return the stable dotted identifier for this variant.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Files => "extract.over_limit.files",
            Self::TotalBytes => "extract.over_limit.total_bytes",
            Self::PathBytes => "extract.over_limit.path_bytes",
            Self::ComponentBytes => "extract.over_limit.component_bytes",
            Self::Depth => "extract.over_limit.depth",
        }
    }
}

/// Why an inventory could not be turned into an extraction plan.
///
/// A planning failure rejects the whole plan. Nothing is ever partially
/// planned, skipped or renamed: an inventory that cannot be extracted safely in
/// full is refused in full, which is the only outcome that keeps a later
/// `extract` command from reporting success over a silently reduced result.
///
/// The enum is `#[non_exhaustive]`: match on [`PlanError::code`] for stable
/// behaviour across versions, and treat an unknown code as a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PlanError {
    /// The entry is one this crate refuses to plan output for.
    Unsupported {
        /// Which kind of entry.
        kind: UnsupportedEntryKind,
        /// Central-directory index of the entry.
        entry: u32,
    },
    /// A destination component is unsafe for any filesystem.
    UnsafePath {
        /// Which unsafe shape. The component itself is never reported.
        kind: UnsafeComponentKind,
        /// Central-directory index of the entry.
        entry: u32,
    },
    /// Two planned outputs cannot both exist.
    Ambiguous {
        /// Which ambiguity.
        kind: PlanAmbiguityKind,
        /// Central-directory index of the later of the two entries.
        entry: u32,
    },
    /// A documented extraction limit was exceeded.
    OverLimit {
        /// Which limit.
        limit: ExtractLimitKind,
        /// The configured ceiling.
        limit_value: u64,
        /// The value that exceeded it, when meaningful.
        observed: Option<u64>,
        /// Central-directory index of the entry, when entry-scoped.
        entry: Option<u32>,
    },
}

impl PlanError {
    /// Return the stable dotted identifier for this error.
    ///
    /// ```
    /// use openkrx_core::extract::{PlanError, UnsupportedEntryKind};
    ///
    /// let error = PlanError::Unsupported {
    ///     kind: UnsupportedEntryKind::Link,
    ///     entry: 0,
    /// };
    /// assert_eq!(error.code(), "extract.unsupported.link");
    /// ```
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match *self {
            Self::Unsupported { kind, .. } => kind.code(),
            Self::UnsafePath { kind, .. } => kind.code(),
            Self::Ambiguous { kind, .. } => kind.code(),
            Self::OverLimit { limit, .. } => limit.code(),
        }
    }

    /// Central-directory index of the entry the error concerns, if any.
    #[must_use]
    pub const fn entry_index(&self) -> Option<u32> {
        match *self {
            Self::Unsupported { entry, .. }
            | Self::UnsafePath { entry, .. }
            | Self::Ambiguous { entry, .. } => Some(entry),
            Self::OverLimit { entry, .. } => entry,
        }
    }
}

impl fmt::Display for PlanError {
    /// Print the code, entry index and numeric limit values only.
    ///
    /// Entry names, destination components and archive content are deliberately
    /// excluded, so a planning diagnostic can be logged wherever an inventory
    /// diagnostic can.
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
        if let Some(entry) = self.entry_index() {
            write!(f, " at entry {entry}")?;
        }
        Ok(())
    }
}

impl std::error::Error for PlanError {}
