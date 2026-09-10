//! Deterministic editing of a package that already exists.
//!
//! Repacking is the composition of the three layers this crate already has:
//! [`crate::archive::inventory`] reads the package, [`crate::metadata::parse`]
//! turns its document into typed values, and [`crate::create::package`] writes
//! the result. [`plan`] decides what an [`Edits`] value would do to a package
//! and [`apply`] carries it out, both without touching a filesystem, a clock,
//! a process or a network.
//!
//! # What is preserved
//!
//! An attachment no edit names is carried through **byte for byte**: its
//! entry's decoded bytes go into the result unchanged, and so do its
//! `MELLEKLET_LEIRASA`, `MENNYISEG` and `MENNYISEGI_EGYSEG`. Its number and
//! location are re-derived, because removing an earlier attachment renumbers
//! the ones after it and a document that described the old numbering would
//! describe a package that no longer exists.
//!
//! # What is refused, and why that is the point
//!
//! The writer emits one layout and one grammar. Anything the input carries
//! that it cannot re-emit is refused with a `repack.unsupported.*` code rather
//! than dropped:
//!
//! | Refused | Because |
//! | --- | --- |
//! | A root prefix other than `KRX/OCD/`, or another spelling of the metadata file | Rules A19 and M12 leave both open; re-emitting would move or rename entries |
//! | An entry that is not the marker, the document or `KRX/OCD/Payload/ID-<n>/<file>` | `signatures.xml` (A7) and a service document (A11–A16) would be dropped |
//! | `unknown_elements > 0` | The reader counts elements outside the grammar (A9) and keeps none |
//! | `ERKEZTETES`, `BONTASOK`, `TERTIVEVENY`, an unqualified `KEZELESI_UTASITASOK` | The reader records their presence only, so they would be written back empty |
//! | More than one `EXPEDIALAS` block | There is no single place for the derived references (M7) |
//! | A reference the writer would derive differently | Repacking would rewrite a `MERET`, a location or a number nobody asked it to |
//!
//! `MELLEKLETEK_SZAMA` is refused in both directions for the same reason: the
//! writer derives the count and always emits it, so a dispatch that declared
//! none would gain one.
//!
//! The `MELLEKLETEK` container is not refused, because it does not have to be:
//! the reader retains whether the dispatch carried the element and the writer
//! emits exactly what it was told, so a dispatch that carried no container
//! keeps none instead of gaining an empty one.
//!
//! The rule behind every other row is one sentence: **repacking with no edit
//! must produce the package it was given.** A package this module accepts is one
//! openKRX could have written itself, which is why the refusals are a feature
//! rather than a limitation — openKRX will not hand back a package that
//! quietly lost part of someone's correspondence.
//!
//! Nothing here signs, encrypts or verifies anything, and a repacked package is
//! "structurally consistent with the documented layout", never "conforming".
//!
//! ```
//! use openkrx_core::repack::{self, Edits};
//! use openkrx_core::{Limits, MetadataLimits, archive, create::FixedTimestamp};
//!
//! # fn demo(image: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
//! let inventory = archive::inventory(image, &Limits::DEFAULT)?;
//! let mut edits = Edits::default();
//! edits.header.consignment_id = Some("REPLACEMENT-ID".to_owned());
//! let plan = repack::plan(&inventory, &Limits::DEFAULT, &MetadataLimits::DEFAULT, &edits)?;
//! let _ = (plan.preserved(), plan.changed(), plan.added(), plan.removed());
//! let bytes = repack::apply(&inventory, &plan, FixedTimestamp::EPOCH, &Limits::DEFAULT)?;
//! # let _ = bytes;
//! # Ok(())
//! # }
//! ```

mod edits;
mod error;
mod layout;
mod plan;

use crate::archive::ArchiveInventory;
use crate::create::{self, FixedTimestamp, PackageSpec};
use crate::limits::Limits;
use crate::metadata::MetadataLimits;

pub use edits::{
    AttachmentAddition, AttachmentReplacement, Edits, HeaderEdits, HeaderField, OptionalEdit,
};
pub use error::{InvalidKind, RepackError, UnsupportedKind};
pub use plan::RepackPlan;

/// Decide what repacking this package under `edits` would produce.
///
/// The result is inspectable before anything is written: it lists the
/// attachments that stay byte-identical, the ones whose bytes change, the ones
/// that go, the numbers the new ones take, and the header fields the edits set.
///
/// What this function decides is the input and the shape of the result: that
/// the package is in the layout the writer emits, that nothing it carries
/// would be lost or rewritten, that every edit names an attachment the package
/// actually holds and names it once, and that the result stays inside the
/// entry-count and per-entry ceilings. A plan therefore says the *package* can
/// carry these edits.
///
/// It is not the whole refusal surface. The writer's own rules — the name
/// classes an attachment file name must satisfy, the characters XML 1.0 can
/// carry, the text the reader would trim, and the remaining ceilings — apply
/// to the values an edit supplies, and they are enforced by
/// [`crate::create::package`] inside [`apply`], reported as `create.*`. An
/// added attachment with an unsafe file name is the ordinary case: it plans
/// and then refuses.
///
/// # Errors
///
/// A [`RepackError`] whose [`code`](RepackError::code) distinguishes an input
/// the writer cannot re-emit (`repack.unsupported.*`), an edit naming an
/// attachment the package does not hold (`repack.invalid.*`), a document that
/// could not be parsed (`metadata.*`), an entry that could not be re-read
/// (`archive.*`) and a result that would exceed a ceiling
/// (`create.over_limit.*`).
pub fn plan(
    inventory: &ArchiveInventory<'_>,
    limits: &Limits,
    metadata_limits: &MetadataLimits,
    edits: &Edits,
) -> Result<RepackPlan, RepackError> {
    plan::plan(inventory, limits, metadata_limits, edits)
}

/// Write the package `plan` describes and return its bytes.
///
/// `inventory` must be the one the plan was made from: every preserved
/// attachment's bytes are re-read from it, and an inventory whose entry count
/// or CRC-32 values differ is refused with `repack.invalid.inventory_mismatch`
/// rather than used to write bytes the plan does not describe.
///
/// `timestamp` is the caller's, exactly as it is for
/// [`crate::create::package`]: this crate has no clock, and the same plan with
/// the same timestamp writes the same bytes.
///
/// # Errors
///
/// A [`RepackError`]: `archive.*` when a preserved entry could not be re-read,
/// `repack.invalid.inventory_mismatch` for an inventory the plan was not made
/// from, and `create.*` when the writer refuses the package the plan
/// describes.
pub fn apply(
    inventory: &ArchiveInventory<'_>,
    plan: &RepackPlan,
    timestamp: FixedTimestamp,
    limits: &Limits,
) -> Result<Vec<u8>, RepackError> {
    let spec = PackageSpec {
        attachments: plan.inputs(inventory)?,
        timestamp,
        ..PackageSpec::new(plan.metadata().clone())
    };
    Ok(create::package(&spec, limits)?)
}
