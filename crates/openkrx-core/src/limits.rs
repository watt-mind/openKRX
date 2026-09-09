//! Resource limits for bounded archive processing.
//!
//! Every limit is enforced against decoded bytes rather than attacker-supplied
//! header declarations, as required by the project security policy. The values
//! in [`Limits::DEFAULT`] are published in `docs/architecture.md`; changing them
//! requires updating that document.

/// Byte, count and ratio ceilings applied while reading an archive.
///
/// All fields are public so a caller can tighten (or, deliberately, relax) an
/// individual bound. The struct is [`Copy`] and carries no interior state, so
/// the same value can be shared across calls.
///
/// ```
/// use openkrx_core::Limits;
///
/// let mut limits = Limits::DEFAULT;
/// limits.max_entries = 8;
/// assert_eq!(Limits::DEFAULT.max_entries, 256);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Largest accepted archive image, in bytes.
    pub max_archive_bytes: u64,
    /// Largest accepted number of central-directory records.
    pub max_entries: u32,
    /// Largest accepted entry-name length, in bytes.
    pub max_name_bytes: u32,
    /// Largest accepted decoded size of a single entry, in bytes.
    pub max_entry_decoded_bytes: u64,
    /// Largest accepted decoded size of all entries together, in bytes.
    pub max_total_decoded_bytes: u64,
    /// Largest accepted decoded-to-compressed ratio for a single entry.
    ///
    /// Only checked once the entry has produced more than
    /// [`Limits::RATIO_GRACE_BYTES`] decoded bytes, so that tiny entries with a
    /// necessarily poor ratio are not rejected.
    pub max_compression_ratio: u64,
    /// Largest accepted extra-field block, in bytes, per header.
    pub max_extra_field_bytes: u32,
    /// Largest accepted comment, in bytes, per record and for the archive.
    pub max_comment_bytes: u32,
}

impl Limits {
    /// Decoded bytes an entry may produce before the ratio check starts.
    pub const RATIO_GRACE_BYTES: u64 = 64 * 1024;

    /// Size of the single bounded inflate output buffer, in bytes.
    ///
    /// This is the granularity at which streaming limits are enforced: a limit
    /// can never be exceeded by more than one buffer before decoding aborts.
    pub const OUTPUT_BUFFER_BYTES: usize = 64 * 1024;

    /// Documented default limits.
    ///
    /// | Limit | Value |
    /// | --- | --- |
    /// | `max_archive_bytes` | 64 MiB |
    /// | `max_entries` | 256 |
    /// | `max_name_bytes` | 255 |
    /// | `max_entry_decoded_bytes` | 32 MiB |
    /// | `max_total_decoded_bytes` | 128 MiB |
    /// | `max_compression_ratio` | 100 |
    /// | `max_extra_field_bytes` | 4 KiB |
    /// | `max_comment_bytes` | 1 KiB |
    pub const DEFAULT: Self = Self {
        max_archive_bytes: 64 * 1024 * 1024,
        max_entries: 256,
        max_name_bytes: 255,
        max_entry_decoded_bytes: 32 * 1024 * 1024,
        max_total_decoded_bytes: 128 * 1024 * 1024,
        max_compression_ratio: 100,
        max_extra_field_bytes: 4 * 1024,
        max_comment_bytes: 1024,
    };
}

impl Default for Limits {
    fn default() -> Self {
        Self::DEFAULT
    }
}
