//! Resource limits for bounded metadata parsing.
//!
//! Every limit is enforced inside the XML event loop, against bytes and counts
//! actually produced, never against a declaration inside the document. The
//! values in [`MetadataLimits::DEFAULT`] are published in
//! `docs/architecture.md`; changing one requires updating that table in the
//! same commit.

/// Byte and count ceilings applied while reading a metadata document.
///
/// All fields are public so a caller can tighten (or, deliberately, relax) an
/// individual bound. The struct is [`Copy`] and carries no interior state.
///
/// ```
/// use openkrx_core::{Limits, MetadataLimits};
///
/// let mut limits = MetadataLimits::DEFAULT;
/// limits.max_elements = 64;
/// // The document ceiling deliberately equals the archive's per-entry ceiling:
/// // the document is read through `ArchiveInventory::entry_bytes`.
/// assert_eq!(
///     MetadataLimits::DEFAULT.max_document_bytes,
///     Limits::DEFAULT.max_entry_decoded_bytes
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetadataLimits {
    /// Largest accepted metadata document, in bytes.
    pub max_document_bytes: u64,
    /// Largest accepted element nesting depth, counting the root as depth 1.
    pub max_depth: u32,
    /// Largest accepted number of elements in one document.
    pub max_elements: u32,
    /// Largest accepted number of attributes on one element.
    pub max_attributes_per_element: u32,
    /// Largest accepted total of character data, in bytes, across the document.
    pub max_text_bytes: u64,
}

impl MetadataLimits {
    /// Documented default limits.
    ///
    /// | Limit | Value |
    /// | --- | --- |
    /// | `max_document_bytes` | 32 MiB |
    /// | `max_depth` | 32 |
    /// | `max_elements` | 10 000 |
    /// | `max_attributes_per_element` | 32 |
    /// | `max_text_bytes` | 1 MiB |
    ///
    /// `max_document_bytes` matches `Limits::DEFAULT.max_entry_decoded_bytes`,
    /// because the document reaches this layer through
    /// `ArchiveInventory::entry_bytes` and cannot be larger than that ceiling.
    pub const DEFAULT: Self = Self {
        max_document_bytes: 32 * 1024 * 1024,
        max_depth: 32,
        max_elements: 10_000,
        max_attributes_per_element: 32,
        max_text_bytes: 1024 * 1024,
    };
}

impl Default for MetadataLimits {
    fn default() -> Self {
        Self::DEFAULT
    }
}
