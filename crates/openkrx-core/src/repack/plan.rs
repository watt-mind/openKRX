//! The plan a repacking run produces, and the planning itself.
//!
//! A [`RepackPlan`] is the whole decision, taken before a byte is written: what
//! is preserved, what changes, what goes and what arrives, and which header
//! fields the edits set. It is inspectable on purpose — a caller repacking
//! someone's correspondence is entitled to see what will happen to it first —
//! and [`super::apply`] adds nothing to it.

use crate::archive::ArchiveInventory;
use crate::create::AttachmentInput;
use crate::limits::Limits;
use crate::metadata::{AttachmentReference, Dispatch, Metadata, MetadataLimits};

use super::edits::{Edits, HeaderField};
use super::error::{InvalidKind, RepackError, UnsupportedKind};
use super::layout::{self, Payload};

/// Where one attachment of the result comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Source {
    /// An entry of the package being repacked, carried through byte for byte.
    Preserved {
        /// Central-directory index in the inventory the plan was made from.
        entry: u32,
        /// The CRC-32 that entry declared, so applying the plan to a different
        /// inventory is refused rather than silently writing other bytes.
        crc32: u32,
    },
    /// Bytes the edits supplied, for an added or a replaced attachment.
    Supplied(Vec<u8>),
}

/// One attachment of the result, in output order.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Planned {
    source: Source,
    file_name: String,
    description: Option<String>,
    quantity: Option<String>,
    quantity_unit: Option<String>,
}

/// What repacking will do, decided before anything is written.
///
/// The four lists count attachments by their `CSATOLMANY_SZAMA`:
/// [`preserved`](RepackPlan::preserved), [`changed`](RepackPlan::changed) and
/// [`removed`](RepackPlan::removed) name numbers in the package being edited,
/// and [`added`](RepackPlan::added) names the numbers the new attachments get
/// in the result. Removing an attachment renumbers the ones after it, which is
/// why the two sides are reported separately rather than as one list.
///
/// The file names and descriptions the plan carries are package content. The
/// `Debug` representation prints them verbatim and must never be logged,
/// persisted or sent through telemetry.
#[derive(Debug, Clone, PartialEq)]
pub struct RepackPlan {
    preserved: Vec<u32>,
    changed: Vec<u32>,
    removed: Vec<u32>,
    added: Vec<u32>,
    header_fields: Vec<HeaderField>,
    attachments: Vec<Planned>,
    metadata: Metadata,
    source_entries: usize,
}

impl RepackPlan {
    /// Numbers of the attachments carried through byte for byte.
    #[must_use]
    pub fn preserved(&self) -> &[u32] {
        &self.preserved
    }

    /// Numbers of the attachments whose bytes the edits replace.
    #[must_use]
    pub fn changed(&self) -> &[u32] {
        &self.changed
    }

    /// Numbers of the attachments the edits remove.
    #[must_use]
    pub fn removed(&self) -> &[u32] {
        &self.removed
    }

    /// The numbers added attachments take in the result.
    #[must_use]
    pub fn added(&self) -> &[u32] {
        &self.added
    }

    /// The header fields the edits set, in a fixed order.
    #[must_use]
    pub fn header_fields(&self) -> &[HeaderField] {
        &self.header_fields
    }

    /// How many attachments the result carries.
    #[must_use]
    pub fn attachment_count(&self) -> usize {
        self.attachments.len()
    }

    /// How many archive entries the result carries: the marker, the metadata
    /// document, and one per attachment.
    #[must_use]
    pub fn entry_count(&self) -> usize {
        self.attachments.len() + 2
    }

    /// The document the result carries, with no derived reference in it yet.
    #[must_use]
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    /// Build the writer's attachment list, taking preserved bytes from
    /// `inventory`.
    pub(super) fn inputs(
        &self,
        inventory: &ArchiveInventory<'_>,
    ) -> Result<Vec<AttachmentInput>, RepackError> {
        if inventory.len() != self.source_entries {
            return Err(mismatched_inventory());
        }
        let mut inputs = Vec::with_capacity(self.attachments.len());
        for attachment in &self.attachments {
            let bytes = match &attachment.source {
                Source::Preserved { entry, crc32 } => {
                    let declared = inventory
                        .entries()
                        .get(*entry as usize)
                        .ok_or_else(mismatched_inventory)?;
                    if declared.crc32() != *crc32 {
                        return Err(mismatched_inventory());
                    }
                    inventory.entry_bytes(*entry)?
                }
                Source::Supplied(bytes) => bytes.clone(),
            };
            inputs.push(AttachmentInput {
                file_name: attachment.file_name.clone(),
                bytes,
                description: attachment.description.clone(),
                quantity: attachment.quantity.clone(),
                quantity_unit: attachment.quantity_unit.clone(),
            });
        }
        Ok(inputs)
    }
}

/// The refusal for an inventory that is not the one the plan was made from.
fn mismatched_inventory() -> RepackError {
    RepackError::Invalid {
        kind: InvalidKind::InventoryMismatch,
        number: None,
    }
}

/// Decide what repacking `inventory` under `edits` would produce.
///
/// # Errors
///
/// See [`super::plan`], which is this function.
pub(super) fn plan(
    inventory: &ArchiveInventory<'_>,
    limits: &Limits,
    metadata_limits: &MetadataLimits,
    edits: &Edits,
) -> Result<RepackPlan, RepackError> {
    let canonical = layout::read(inventory)?;
    let document = inventory.entry_bytes(canonical.metadata_entry)?;
    let metadata = crate::metadata::parse(&document, metadata_limits)?;
    check_reproducible(&metadata)?;
    let references = references(&metadata, &canonical.payloads)?;
    check_references(inventory, &canonical.payloads, &references)?;
    check_targets(edits, references.len())?;
    build(
        inventory,
        limits,
        edits,
        metadata,
        &canonical.payloads,
        references,
    )
}

/// Refuse a document carrying anything the writer cannot re-emit.
///
/// Each of these is content the reader observed but did not retain: an element
/// outside the grammar is counted (A9), and `ERKEZTETES`, `BONTASOK`,
/// `TERTIVEVENY` and an unqualified `KEZELESI_UTASITASOK` are recorded as
/// present and never read further. Writing the document again would emit them
/// empty, or not at all, which is a change nobody asked for.
fn check_reproducible(metadata: &Metadata) -> Result<(), RepackError> {
    let refuse = |kind| {
        Err(RepackError::Unsupported {
            kind,
            entry: None,
            number: None,
        })
    };
    if metadata.unknown_elements != 0 {
        return refuse(UnsupportedKind::UnknownElements);
    }
    if metadata.receipt_present || metadata.openings_present || metadata.return_receipt_present {
        return refuse(UnsupportedKind::OpaqueBlock);
    }
    if metadata
        .dispatches
        .iter()
        .any(|dispatch| dispatch.handling_instructions_unqualified)
    {
        return refuse(UnsupportedKind::OpaqueBlock);
    }
    if metadata.dispatches.len() > 1 {
        return refuse(UnsupportedKind::DispatchCount);
    }
    Ok(())
}

/// The document's attachment references, checked against what a writer derives.
///
/// The rule is one sentence: repacking a package with no edit at all must
/// produce that package again. A reference the writer would rewrite — another
/// number, another location, another `MERET` — therefore refuses the package
/// instead of being quietly corrected.
///
/// `MELLEKLETEK_SZAMA` is checked in both directions for the same reason. The
/// schema requires it beside the list and M7 makes the two a check rather than
/// an assumption, so the reader records an absent element as `None` — but the
/// writer derives the count from the attachments and always emits it. A
/// document that omitted the element would therefore *gain* one, which is a
/// change nobody asked for, so a dispatch that does not declare a count is
/// refused exactly as one that declares the wrong count is.
fn references(
    metadata: &Metadata,
    payloads: &[Payload],
) -> Result<Vec<AttachmentReference>, RepackError> {
    let references: Vec<AttachmentReference> = metadata.attachments().cloned().collect();
    if references.len() != payloads.len() {
        return Err(RepackError::Unsupported {
            kind: UnsupportedKind::AttachmentEntry,
            entry: None,
            number: None,
        });
    }
    let counted = i64::try_from(references.len()).ok();
    // At most one dispatch reaches this: `check_reproducible` refused more.
    // A document with no dispatch at all declares no count and gets none
    // written, so there is nothing to compare.
    if let Some(dispatch) = metadata.dispatches.first()
        && (dispatch.declared_attachment_count.is_none()
            || dispatch.declared_attachment_count != counted)
    {
        return Err(RepackError::Unsupported {
            kind: UnsupportedKind::AttachmentReference,
            entry: None,
            number: None,
        });
    }
    Ok(references)
}

/// Every reference must describe exactly the entry the layout puts it in.
fn check_references(
    inventory: &ArchiveInventory<'_>,
    payloads: &[Payload],
    references: &[AttachmentReference],
) -> Result<(), RepackError> {
    for (index, (reference, payload)) in references.iter().zip(payloads).enumerate() {
        let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
        let refuse = |kind| {
            Err(RepackError::Unsupported {
                kind,
                entry: Some(payload.entry),
                number: Some(number),
            })
        };
        if reference.file_name != payload.file_name {
            return refuse(UnsupportedKind::AttachmentEntry);
        }
        let bytes = inventory
            .entries()
            .get(payload.entry as usize)
            .map_or(0, |entry| entry.decoded_size());
        if reference.number != i64::from(number)
            || reference.location != payload_location(number)
            || reference.size_text != declared_kilobytes(bytes)
        {
            return refuse(UnsupportedKind::AttachmentReference);
        }
    }
    Ok(())
}

/// `ELHELYEZKEDES` for attachment `number`, as the writer derives it (A5, A22).
fn payload_location(number: u32) -> String {
    format!("{}{number}", crate::create::PAYLOAD_PREFIX)
}

/// `MERET` for a payload of `bytes` bytes, as the writer derives it (M6).
fn declared_kilobytes(bytes: u64) -> String {
    bytes.div_ceil(1024).to_string()
}

/// Refuse an edit that names an attachment the package does not carry, or
/// names one twice.
///
/// Both are refused before anything is planned, because the alternative is a
/// caller believing an edit was applied when the package silently did not have
/// what the edit named.
fn check_targets(edits: &Edits, attachments: usize) -> Result<(), RepackError> {
    let count = u32::try_from(attachments).unwrap_or(u32::MAX);
    let mut seen: Vec<u32> = Vec::with_capacity(edits.remove.len() + edits.replace.len());
    let targets = edits
        .remove
        .iter()
        .copied()
        .chain(edits.replace.iter().map(|replacement| replacement.number));
    for number in targets {
        let refuse = |kind| {
            Err(RepackError::Invalid {
                kind,
                number: Some(number),
            })
        };
        if number == 0 || number > count {
            return refuse(InvalidKind::NoSuchAttachment);
        }
        if seen.contains(&number) {
            return refuse(InvalidKind::DuplicateTarget);
        }
        seen.push(number);
    }
    Ok(())
}

/// Assemble the plan itself, once every refusal has been decided.
fn build(
    inventory: &ArchiveInventory<'_>,
    limits: &Limits,
    edits: &Edits,
    metadata: Metadata,
    payloads: &[Payload],
    references: Vec<AttachmentReference>,
) -> Result<RepackPlan, RepackError> {
    let mut plan = RepackPlan {
        preserved: Vec::new(),
        changed: Vec::new(),
        removed: edits.remove.clone(),
        added: Vec::new(),
        header_fields: edits.header.fields(),
        attachments: Vec::new(),
        metadata,
        source_entries: inventory.len(),
    };
    plan.removed.sort_unstable();
    for (index, (reference, payload)) in references.iter().zip(payloads).enumerate() {
        let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
        if edits.remove.contains(&number) {
            continue;
        }
        let replacement = edits
            .replace
            .iter()
            .find(|replacement| replacement.number == number);
        let source = match replacement {
            Some(replacement) => {
                plan.changed.push(number);
                Source::Supplied(replacement.bytes.clone())
            }
            None => {
                plan.preserved.push(number);
                Source::Preserved {
                    entry: payload.entry,
                    crc32: payload.crc32,
                }
            }
        };
        plan.attachments.push(Planned {
            source,
            file_name: payload.file_name.clone(),
            description: reference.description.clone(),
            quantity: reference.quantity.clone(),
            quantity_unit: reference.quantity_unit.clone(),
        });
    }
    for addition in &edits.add {
        let number = u32::try_from(plan.attachments.len() + 1).unwrap_or(u32::MAX);
        plan.added.push(number);
        plan.attachments.push(Planned {
            source: Source::Supplied(addition.bytes.clone()),
            file_name: addition.file_name.clone(),
            description: addition.description.clone(),
            quantity: None,
            quantity_unit: None,
        });
    }
    edits.header.apply(&mut plan.metadata.header);
    place_dispatch(&mut plan);
    check_size(&plan, limits)?;
    Ok(plan)
}

/// Leave the document one `EXPEDIALAS` block to derive references into.
///
/// The references and the count are cleared rather than edited: the writer
/// derives both from the attachments it is actually given, so a stale list
/// carried over from the package being edited would describe the wrong one.
fn place_dispatch(plan: &mut RepackPlan) {
    if plan.attachments.is_empty() {
        for dispatch in &mut plan.metadata.dispatches {
            dispatch.declared_attachment_count = Some(0);
            dispatch.attachments = Vec::new();
        }
        return;
    }
    if plan.metadata.dispatches.is_empty() {
        plan.metadata.dispatches.push(Dispatch {
            declared_attachment_count: None,
            attachments: Vec::new(),
            handling_instructions_unqualified: false,
        });
    }
    for dispatch in &mut plan.metadata.dispatches {
        dispatch.declared_attachment_count = None;
        dispatch.attachments = Vec::new();
    }
}

/// Refuse a result that would exceed a documented ceiling, before writing it.
///
/// The writer applies every ceiling again over the bytes it assembles; these
/// two are checked here as well so that a plan a caller inspects is one that
/// can be applied, rather than one that fails at the last moment.
fn check_size(plan: &RepackPlan, limits: &Limits) -> Result<(), RepackError> {
    use crate::create::{CreateError, CreateLimitKind};

    let entries = u64::try_from(plan.entry_count()).unwrap_or(u64::MAX);
    let ceiling = u64::from(limits.max_entries).min(u64::from(u16::MAX));
    if entries > ceiling {
        return Err(RepackError::Write(CreateError::OverLimit {
            limit: CreateLimitKind::Entries,
            limit_value: ceiling,
            observed: Some(entries),
            index: None,
        }));
    }
    let entry_ceiling = limits.max_entry_decoded_bytes.min(u64::from(u32::MAX));
    for (index, attachment) in plan.attachments.iter().enumerate() {
        let Source::Supplied(bytes) = &attachment.source else {
            continue;
        };
        let length = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if length > entry_ceiling {
            return Err(RepackError::Write(CreateError::OverLimit {
                limit: CreateLimitKind::EntryBytes,
                limit_value: entry_ceiling,
                observed: Some(length),
                index: u32::try_from(index).ok(),
            }));
        }
    }
    Ok(())
}
