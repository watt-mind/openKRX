//! Deterministic package creation in the canonical documented layout.
//!
//! [`package`] turns a [`PackageSpec`] into the bytes of one archive. It is a
//! pure function: no filesystem, clock, process or network access, no
//! randomness, no environment. Equal inputs produce byte-identical output, and
//! the only time in the result is the [`FixedTimestamp`] the caller supplied.
//!
//! # What is written, and what that does not mean
//!
//! The writer emits the canonical documented layout. Interoperability with real
//! producers is unverified because rules A19–A22 and M11–M15 remain unresolved;
//! the reader's structural checks are the only gate, and a written package is
//! "structurally consistent with the documented layout", never "conforming".
//!
//! Concretely, the entries are, in this order:
//!
//! | Entry | Content | Method |
//! | --- | --- | --- |
//! | `KRX/OCD/mimetype` | `application/OCD+ZIP` (A2) | stored |
//! | `KRX/OCD/Metalayer/KULDEMENY_META.xml` | the serialised document (A4) | deflate |
//! | `KRX/OCD/Payload/ID-<n>/<file>` | one attachment, verbatim (A5) | deflate |
//!
//! A package written this way passes [`crate::archive::inventory`],
//! [`crate::metadata::parse`] and [`crate::profile::check`] with no
//! [`crate::profile::CheckOutcome::Fail`]. It does **not** pass them with no
//! `Unresolved`: the marker sits under the `KRX/OCD/` prefix, which is exactly
//! what rule A19 leaves open, and the declared attachment size cites M13. Those
//! outcomes are the honest report of an unresolved rule, and
//! [`verify_round_trip`] returns the report so a caller can assert them rather
//! than be told a package is fine.
//!
//! Nothing here signs, encrypts or writes `signatures.xml` (A7), and nothing
//! writes a service-specific document such as `DeliveryInstruction.xml`
//! (A11–A16). openKRX performs no cryptography at all.
//!
//! # Determinism
//!
//! Fixed: the entry order, the compression method per entry, the deflate level,
//! the UTF-8 name flag on every entry, `version made by` (host 3) with mode
//! `0o100644`, the absence of directory entries, extra fields, data descriptors
//! and comments, and the element order and escaping of the document. The full
//! table is in `docs/architecture.md#deterministic-creation`.
//!
//! ```
//! use openkrx_core::create::{self, AttachmentInput, PackageSpec};
//! use openkrx_core::{Limits, MetadataLimits, profile};
//!
//! # fn demo(metadata: openkrx_core::metadata::Metadata) -> Result<(), Box<dyn std::error::Error>> {
//! let spec = PackageSpec::with_attachments(
//!     metadata,
//!     vec![AttachmentInput::new("report.pdf", b"%PDF-1.7\n".to_vec())],
//! );
//! let bytes = create::package(&spec, &Limits::DEFAULT)?;
//! assert_eq!(bytes, create::package(&spec, &Limits::DEFAULT)?);
//!
//! let report = create::verify_round_trip(&bytes, &Limits::DEFAULT, &MetadataLimits::DEFAULT)?;
//! assert!(report.checks().iter().all(|check| !matches!(
//!     check.outcome,
//!     profile::CheckOutcome::Fail(_)
//! )));
//! # Ok(())
//! # }
//! ```

mod error;
mod names;
mod spec;
mod xml;
mod zip;

use crate::archive::{self, ArchiveInventory};
use crate::limits::Limits;
use crate::metadata::MetadataLimits;
use crate::profile::{self, ProfileError, StructureReport};

pub use error::{CreateError, CreateLimitKind, InvalidKind, UnsafeNameKind};
pub use spec::{AttachmentInput, FixedTimestamp, Layout, PackageSpec};

use zip::{Entry, Method};

/// The archive-root prefix the canonical documented layout uses (A19).
pub const ROOT_PREFIX: &str = "KRX/OCD/";
/// The format marker entry's name in that layout (A2, A19).
pub const MARKER_NAME: &str = "KRX/OCD/mimetype";
/// The format marker entry's content (A2).
pub const MARKER_CONTENT: &[u8] = b"application/OCD+ZIP";
/// The metadata document's entry name (A4, M12).
pub const METADATA_NAME: &str = "KRX/OCD/Metalayer/KULDEMENY_META.xml";
/// The prefix of an attachment's directory; the 1-based number follows (A5).
pub const PAYLOAD_PREFIX: &str = "KRX/OCD/Payload/ID-";

/// Write one package and return its bytes.
///
/// The `MELLEKLET` references and `MELLEKLETEK_SZAMA` of the document are
/// derived from `spec.attachments`: the number is the 1-based index,
/// `ELHELYEZKEDES` is `KRX/OCD/Payload/ID-<n>`, `FAJL_NEV` is the file name and
/// `MERET` is the size in kilobytes, rounded up, written as a numeric string
/// because that is the unit rule M6 documents. What a receiving service
/// actually expects there is unresolved rule M13, and stays unresolved.
///
/// `limits` bounds the output exactly as it bounds reading: a package this
/// function returns is one [`crate::archive::inventory`] accepts under the same
/// value.
///
/// # Errors
///
/// A [`CreateError`] whose [`code`](CreateError::code) distinguishes a request
/// that contradicts itself (`create.invalid.*`), a ceiling the output would
/// exceed (`create.over_limit.*`) and a name this crate refuses to write
/// (`create.unsafe_name.*`). Nothing is written partially: a refusal refuses
/// the whole package.
pub fn package(spec: &PackageSpec, limits: &Limits) -> Result<Vec<u8>, CreateError> {
    match spec.layout {
        // One layout exists. The match is here so that adding a second one
        // fails to compile until this function decides what it writes.
        Layout::CanonicalDocumented => {}
    }
    names::check_entry_name(MARKER_NAME, None)?;
    names::check_entry_name(METADATA_NAME, None)?;
    let metadata = spec::derive(spec)?;
    let document = xml::serialise(&metadata)?;

    let payload_names: Vec<String> = spec
        .attachments
        .iter()
        .enumerate()
        .map(|(index, attachment)| {
            format!("{}/{}", spec::payload_location(index), attachment.file_name)
        })
        .collect();
    let mut entries = Vec::with_capacity(spec.attachments.len() + 2);
    entries.push(Entry {
        name: MARKER_NAME,
        data: MARKER_CONTENT,
        method: Method::Stored,
    });
    entries.push(Entry {
        name: METADATA_NAME,
        data: &document,
        method: Method::Deflate,
    });
    for (attachment, name) in spec.attachments.iter().zip(&payload_names) {
        entries.push(Entry {
            name,
            data: &attachment.bytes,
            method: Method::Deflate,
        });
    }
    check_limits(&entries, limits)?;

    let prepared = zip::prepare(&entries);
    check_ratios(&prepared, limits)?;
    let image_bytes = zip::image_bytes(&prepared);
    let archive_ceiling = limits.max_archive_bytes.min(u64::from(u32::MAX));
    if image_bytes > archive_ceiling {
        return Err(CreateError::OverLimit {
            limit: CreateLimitKind::ArchiveBytes,
            limit_value: archive_ceiling,
            observed: Some(image_bytes),
            index: None,
        });
    }
    Ok(zip::assemble(&prepared, spec.timestamp))
}

/// Read a written package back and report what the structural checks found.
///
/// This is the writer's own gate, and the only one available: it runs
/// [`crate::archive::inventory`], [`crate::metadata::parse`] and
/// [`crate::profile::check`] over the bytes and hands back the whole report,
/// unreduced. There is deliberately no boolean: a caller decides what an
/// `Unresolved` outcome means to it, and no result of this function is a
/// statement that a package conforms to anything.
///
/// # Errors
///
/// A [`ProfileError`] when the bytes cannot be read as an archive at all. An
/// unmet structural expectation is a [`crate::profile::CheckOutcome`] inside
/// the report, not an error.
pub fn verify_round_trip(
    bytes: &[u8],
    limits: &Limits,
    metadata_limits: &MetadataLimits,
) -> Result<StructureReport, ProfileError> {
    let inventory: ArchiveInventory<'_> = archive::inventory(bytes, limits)?;
    profile::check(&inventory, metadata_limits)
}

/// Refuse an entry whose decoded-to-stored ratio the reader would refuse.
///
/// The rule and the numbers are the reader's, in `archive::inflate`: an entry
/// that produced more than [`Limits::RATIO_GRACE_BYTES`] decoded bytes must not
/// exceed `max_compression_ratio`. It is checked here rather than earlier
/// because the ratio is not known until the entry has been compressed, and it
/// is what keeps the promise on [`package`]: a package this crate writes is one
/// it reads back under the same limits. A highly compressible attachment — a
/// long run of one byte — is the case that reaches it.
fn check_ratios(prepared: &[zip::Prepared<'_>], limits: &Limits) -> Result<(), CreateError> {
    for (position, entry) in prepared.iter().enumerate() {
        let decoded = entry.uncompressed_size();
        let ratio = decoded / entry.compressed_size();
        if decoded > Limits::RATIO_GRACE_BYTES && ratio > limits.max_compression_ratio {
            return Err(CreateError::OverLimit {
                limit: CreateLimitKind::CompressionRatio,
                limit_value: limits.max_compression_ratio,
                observed: Some(ratio),
                index: position
                    .checked_sub(2)
                    .and_then(|attachment| u32::try_from(attachment).ok()),
            });
        }
    }
    Ok(())
}

/// Apply every reader ceiling to the entries about to be written.
///
/// The checks run in a fixed order — entry count, then per entry the name
/// length and the decoded size, then the running total — so one request always
/// reports the same code. The two indices the fixed entries occupy are not
/// attachment positions, so a diagnostic about them carries no index at all.
fn check_limits(entries: &[Entry<'_>], limits: &Limits) -> Result<(), CreateError> {
    let count = entries.len() as u64;
    // A non-ZIP64 end record counts entries in 16 bits, so the writer's own
    // ceiling is the smaller of the configured one and what the record holds:
    // a relaxed `max_entries` must refuse the package, never truncate the count.
    let entries_ceiling = u64::from(limits.max_entries).min(u64::from(u16::MAX));
    if count > entries_ceiling {
        return Err(CreateError::OverLimit {
            limit: CreateLimitKind::Entries,
            limit_value: entries_ceiling,
            observed: Some(count),
            index: None,
        });
    }
    let entry_ceiling = limits.max_entry_decoded_bytes.min(u64::from(u32::MAX));
    let mut total = 0_u64;
    for (position, entry) in entries.iter().enumerate() {
        // The first two entries are the marker and the document; only the rest
        // are attachments a caller can point at.
        let index = position
            .checked_sub(2)
            .and_then(|attachment| u32::try_from(attachment).ok());
        let name_bytes = entry.name.len() as u64;
        if name_bytes > u64::from(limits.max_name_bytes) {
            return Err(CreateError::OverLimit {
                limit: CreateLimitKind::NameBytes,
                limit_value: u64::from(limits.max_name_bytes),
                observed: Some(name_bytes),
                index,
            });
        }
        let decoded = entry.data.len() as u64;
        if decoded > entry_ceiling {
            return Err(CreateError::OverLimit {
                limit: CreateLimitKind::EntryBytes,
                limit_value: entry_ceiling,
                observed: Some(decoded),
                index,
            });
        }
        total = total.saturating_add(decoded);
        if total > limits.max_total_decoded_bytes {
            return Err(CreateError::OverLimit {
                limit: CreateLimitKind::TotalBytes,
                limit_value: limits.max_total_decoded_bytes,
                observed: Some(total),
                index,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! The one ceiling no public call can reach: an entry count above what a
    //! non-ZIP64 end record can express. Writing 65 536 entries to prove it
    //! would cost seconds of compression for a `u16`, so the check itself is
    //! called with entries that carry no data.

    use super::{CreateError, CreateLimitKind, Entry, Limits, Method, check_limits};

    #[test]
    fn the_entry_count_ceiling_is_what_the_end_record_can_count() {
        let mut limits = Limits::DEFAULT;
        limits.max_entries = 70_000;
        let entries: Vec<Entry<'_>> = (0..=u32::from(u16::MAX))
            .map(|_| Entry {
                name: "KRX/OCD/Payload/ID-1/a",
                data: &[],
                method: Method::Deflate,
            })
            .collect();
        assert_eq!(entries.len(), 65_536);
        assert_eq!(
            check_limits(&entries, &limits),
            Err(CreateError::OverLimit {
                limit: CreateLimitKind::Entries,
                limit_value: 65_535,
                observed: Some(65_536),
                index: None,
            }),
            "a relaxed max_entries must refuse the package, not truncate the count"
        );
        assert_eq!(check_limits(&entries[..65_535], &limits), Ok(()));
    }
}
