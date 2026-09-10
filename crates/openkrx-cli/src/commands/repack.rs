//! `repack`: what changed, what did not, and what reading the result said.
//!
//! Like `create`, this report is a record rather than a view: every number in
//! it describes a package that exists, and `unresolved_rules` comes from
//! openKRX reading that package back with `profile::check` before the command
//! reported anything at all.
//!
//! **The four attachment lists are the point of the command.** `preserved`
//! names the attachments carried through byte for byte, `changed` the ones
//! whose bytes the edits replaced, `removed` the ones that are gone — all
//! three by their number in the package that was read — and `added` the
//! numbers the new attachments took in the package that was written. Removing
//! an attachment renumbers the ones after it, which is why the two sides are
//! reported separately rather than as one list.
//!
//! The lists carry numbers, never names: an attachment's file name is package
//! content and belongs in `inspect`'s report, not in this one.
//!
//! Nothing here is a conformance claim. `layout` names the one layout openKRX
//! writes, which is documented and **unverified against any real producer**,
//! and `verified` is `false` in this envelope exactly as it is in every other.

use openkrx_core::StructureReport;
use openkrx_core::repack::RepackPlan;
use serde::Serialize;

use super::unresolved_rules;

/// What `repack` reports when a package was written and read back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RepackData {
    /// Length of the package that was written, in bytes.
    pub bytes_written: u64,
    /// Archive entries in it: the marker, the metadata document, and one per
    /// attachment.
    pub entries: u32,
    /// Numbers of the attachments carried through byte for byte.
    pub preserved: Vec<u32>,
    /// Numbers of the attachments whose bytes the edits replaced.
    pub changed: Vec<u32>,
    /// The numbers the added attachments took in the result.
    pub added: Vec<u32>,
    /// Numbers, in the package that was read, of the attachments removed.
    pub removed: Vec<u32>,
    /// The header fields the edits set, by their manifest spelling. Field
    /// names only: no value an edits document carried is reported.
    pub header_fields: Vec<&'static str>,
    /// Each rule the self-check left undecided, once, in citation order.
    pub unresolved_rules: Vec<&'static str>,
    /// The layout that was written. One exists, and it is unverified.
    pub layout: &'static str,
}

/// Build the report from the plan, the written bytes and the self-check.
#[must_use]
pub fn run(bytes_written: u64, plan: &RepackPlan, report: &StructureReport) -> RepackData {
    RepackData {
        bytes_written,
        entries: u32::try_from(plan.entry_count()).unwrap_or(u32::MAX),
        preserved: plan.preserved().to_vec(),
        changed: plan.changed().to_vec(),
        added: plan.added().to_vec(),
        removed: plan.removed().to_vec(),
        header_fields: plan
            .header_fields()
            .iter()
            .map(|field| field.as_str())
            .collect(),
        unresolved_rules: unresolved_rules(report),
        layout: super::create::LAYOUT,
    }
}
