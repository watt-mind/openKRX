//! Protected extraction planning: what would be written, decided before anything is.
//!
//! [`plan`] turns an [`ArchiveInventory`] into an [`ExtractionPlan`], a
//! description of the files an extraction would create. It is a pure function
//! of the inventory and the limits: it performs no filesystem, clock, process or
//! network access, allocates in proportion to the entry count, and returns the
//! same plan for the same inventory every time.
//!
//! Every safety rule `SECURITY.md` requires of extraction that does not need a
//! filesystem is decided here — link and special-file refusal, path-component
//! safety, Unicode and case collisions, and the extraction ceilings — so each
//! one can be tested exhaustively without touching a disk. What is *not* here is
//! the filesystem half: joining the plan onto a caller-selected destination,
//! no-clobber creation, the interrupted-write cleanup policy, and the `extract`
//! command itself. A plan therefore contains no [`std::path::PathBuf`], no
//! absolute path and no platform separator; it carries destination components as
//! text, and the caller decides what a path is.
//!
//! A failure rejects the whole plan. Nothing is skipped, renamed or partially
//! planned, because a caller that received a quietly reduced plan would extract
//! a package that is not the package it was given.
//!
//! ```
//! use openkrx_core::{Limits, archive, extract::{self, ExtractLimits}};
//!
//! # fn demo(image: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
//! let inventory = archive::inventory(image, &Limits::DEFAULT)?;
//! let plan = extract::plan(&inventory, &ExtractLimits::DEFAULT)?;
//! for item in plan.items() {
//!     let _ = (item.entry_index(), item.components(), item.decoded_size());
//! }
//! # Ok(())
//! # }
//! ```

mod collisions;
mod error;
pub(crate) mod paths;

use crate::archive::{ArchiveInventory, EntryKind};

pub use error::{
    ExtractLimitKind, PlanAmbiguityKind, PlanError, UnsafeComponentKind, UnsupportedEntryKind,
};

/// Ceilings applied while planning an extraction.
///
/// These bound the *output* an extraction would produce, and are independent of
/// the [`crate::Limits`] that bound reading the archive: a caller may accept an
/// archive it will not extract. All fields are public so a caller can tighten
/// (or, deliberately, relax) one bound. The struct is [`Copy`] and carries no
/// interior state.
///
/// ```
/// use openkrx_core::extract::ExtractLimits;
///
/// let mut limits = ExtractLimits::DEFAULT;
/// limits.max_files = 8;
/// assert_eq!(ExtractLimits::DEFAULT.max_files, 256);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtractLimits {
    /// Largest accepted number of planned files.
    pub max_files: u32,
    /// Largest accepted total decoded size of all planned files, in bytes.
    pub max_total_bytes: u64,
    /// Largest accepted destination path, in UTF-8 bytes, separators included.
    pub max_path_bytes: u32,
    /// Largest accepted single path component, in UTF-8 bytes.
    pub max_component_bytes: u32,
    /// Largest accepted number of components in a destination path.
    pub max_depth: u32,
}

impl ExtractLimits {
    /// Documented default limits.
    ///
    /// | Limit | Value |
    /// | --- | --- |
    /// | `max_files` | 256 |
    /// | `max_total_bytes` | 128 MiB |
    /// | `max_path_bytes` | 1024 |
    /// | `max_component_bytes` | 255 |
    /// | `max_depth` | 16 |
    pub const DEFAULT: Self = Self {
        max_files: 256,
        max_total_bytes: 128 * 1024 * 1024,
        max_path_bytes: 1024,
        max_component_bytes: 255,
        max_depth: 16,
    };
}

impl Default for ExtractLimits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// One file an extraction would create.
///
/// `components` is the destination path relative to a destination the caller
/// will choose, split on the archive's only separator. It never holds an empty,
/// relative or reserved component, and joining it is the only step left.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanItem {
    entry_index: u32,
    components: Vec<String>,
    declared_size: u64,
    decoded_size: u64,
}

impl PlanItem {
    /// Central-directory index of the entry this item would write.
    #[must_use]
    pub const fn entry_index(&self) -> u32 {
        self.entry_index
    }

    /// Destination path components, relative to the caller's destination.
    #[must_use]
    pub fn components(&self) -> &[String] {
        &self.components
    }

    /// Uncompressed size the central directory declares for the entry.
    #[must_use]
    pub const fn declared_size(&self) -> u64 {
        self.declared_size
    }

    /// Bytes the inventory actually decoded for the entry.
    ///
    /// The inventory only succeeds when this equals
    /// [`PlanItem::declared_size`]; both are reported so a caller never has to
    /// trust the declaration.
    #[must_use]
    pub const fn decoded_size(&self) -> u64 {
        self.decoded_size
    }
}

/// What an extraction would create, and nothing it has created.
///
/// The plan is a value: producing it wrote nothing, reserved nothing and looked
/// at no filesystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractionPlan {
    items: Vec<PlanItem>,
    total_bytes: u64,
    directories: Vec<Vec<String>>,
    limits_applied: ExtractLimits,
}

impl ExtractionPlan {
    /// Planned files, in central-directory order.
    #[must_use]
    pub fn items(&self) -> &[PlanItem] {
        &self.items
    }

    /// Number of planned files.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the plan would create no file at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Total decoded bytes across every planned file.
    #[must_use]
    pub const fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    /// Implicit parent directories of the planned files, deduplicated and
    /// sorted component-wise, so a parent always precedes its children.
    ///
    /// A directory marker in the archive contributes nothing on its own: an
    /// empty directory is deliberately not materialised.
    #[must_use]
    pub fn directories(&self) -> &[Vec<String>] {
        &self.directories
    }

    /// The limits this plan was produced under.
    #[must_use]
    pub const fn limits_applied(&self) -> ExtractLimits {
        self.limits_applied
    }
}

/// Plan the extraction of `inventory` under `limits`.
///
/// Entries are considered in central-directory order. A directory marker
/// produces no item; a symlink, a special file, a name that is not UTF-8, an
/// unsafe path component, a collision or an exceeded ceiling rejects the plan
/// as a whole.
///
/// # Errors
///
/// Returns a [`PlanError`] whose [`code`](PlanError::code) distinguishes
/// unsupported entries, unsafe paths, ambiguous outputs and exceeded limits. A
/// successful plan is not a statement that the archive is a KRX package, and not
/// a statement that extraction would succeed: no filesystem has been consulted.
///
/// ```
/// use openkrx_core::extract::{ExtractLimits, PlanError};
///
/// let error = PlanError::OverLimit {
///     limit: openkrx_core::extract::ExtractLimitKind::Files,
///     limit_value: ExtractLimits::DEFAULT.max_files.into(),
///     observed: Some(257),
///     entry: Some(256),
/// };
/// assert_eq!(error.code(), "extract.over_limit.files");
/// ```
pub fn plan(
    inventory: &ArchiveInventory<'_>,
    limits: &ExtractLimits,
) -> Result<ExtractionPlan, PlanError> {
    let mut items: Vec<PlanItem> = Vec::with_capacity(inventory.len());
    let mut total_bytes = 0_u64;
    for (index, entry) in inventory.entries().iter().enumerate() {
        let index = index as u32;
        match entry.kind() {
            EntryKind::DirectoryMarker => continue,
            EntryKind::Symlink => return Err(unsupported(UnsupportedEntryKind::Link, index)),
            EntryKind::Special => {
                return Err(unsupported(UnsupportedEntryKind::SpecialFile, index));
            }
            EntryKind::RegularFile | EntryKind::Unknown => {}
        }
        let name = entry
            .name_text()
            .ok_or_else(|| unsupported(UnsupportedEntryKind::NonUtf8Name, index))?;
        let components = destination(name, index, limits)?;
        if items.len() as u64 >= u64::from(limits.max_files) {
            return Err(PlanError::OverLimit {
                limit: ExtractLimitKind::Files,
                limit_value: u64::from(limits.max_files),
                observed: Some(items.len() as u64 + 1),
                entry: Some(index),
            });
        }
        let over_total = |observed| PlanError::OverLimit {
            limit: ExtractLimitKind::TotalBytes,
            limit_value: limits.max_total_bytes,
            observed,
            entry: Some(index),
        };
        match total_bytes.checked_add(entry.decoded_size()) {
            Some(total) if total <= limits.max_total_bytes => total_bytes = total,
            Some(total) => return Err(over_total(Some(total))),
            None => return Err(over_total(None)),
        }
        items.push(PlanItem {
            entry_index: index,
            components,
            declared_size: entry.uncompressed_size(),
            decoded_size: entry.decoded_size(),
        });
    }
    let paths: Vec<(u32, Vec<String>)> = items
        .iter()
        .map(|item| (item.entry_index, item.components.clone()))
        .collect();
    collisions::check(&paths)?;
    let directories = collisions::directories(&paths);
    Ok(ExtractionPlan {
        items,
        total_bytes,
        directories,
        limits_applied: *limits,
    })
}

/// Split one entry name into safe destination components.
///
/// Each component is checked for shape and then for length, in order, so the
/// reported code is always the first violation along the path.
fn destination(name: &str, index: u32, limits: &ExtractLimits) -> Result<Vec<String>, PlanError> {
    if name.len() as u64 > u64::from(limits.max_path_bytes) {
        return Err(PlanError::OverLimit {
            limit: ExtractLimitKind::PathBytes,
            limit_value: u64::from(limits.max_path_bytes),
            observed: Some(name.len() as u64),
            entry: Some(index),
        });
    }
    let parts = paths::components(name);
    if parts.len() as u64 > u64::from(limits.max_depth) {
        return Err(PlanError::OverLimit {
            limit: ExtractLimitKind::Depth,
            limit_value: u64::from(limits.max_depth),
            observed: Some(parts.len() as u64),
            entry: Some(index),
        });
    }
    let mut components = Vec::with_capacity(parts.len());
    for part in parts {
        paths::check_component(part)
            .map_err(|kind| PlanError::UnsafePath { kind, entry: index })?;
        if part.len() as u64 > u64::from(limits.max_component_bytes) {
            return Err(PlanError::OverLimit {
                limit: ExtractLimitKind::ComponentBytes,
                limit_value: u64::from(limits.max_component_bytes),
                observed: Some(part.len() as u64),
                entry: Some(index),
            });
        }
        components.push(part.to_string());
    }
    Ok(components)
}

/// Build an entry-scoped unsupported-entry error.
const fn unsupported(kind: UnsupportedEntryKind, entry: u32) -> PlanError {
    PlanError::Unsupported { kind, entry }
}
