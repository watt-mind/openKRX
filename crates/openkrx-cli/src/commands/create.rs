//! `create`: what was written, and what reading it straight back reported.
//!
//! Like `extract`, and unlike the three reader commands, this report is a
//! record rather than a view: `bytes_written` and `entries` describe a package
//! that exists, and `unresolved_rules` comes from openKRX reading that package
//! back with `profile::check` before the command reported anything at all.
//!
//! **`unresolved_rules` is the honest part of a successful run.** A package
//! openKRX writes always cites at least `A19` — the marker sits under the
//! `KRX/OCD/` prefix and the three primary sources disagree about where it
//! belongs — and cites `M13` as well whenever it carries an attachment, whose
//! `MERET` unit no source settles. That is why `validate-structure` over a
//! package this command just wrote exits `4` and never `3`, and why exit `4`
//! is the definition of success here rather than a defect to chase.
//!
//! Nothing in the report is a conformance claim. `layout` names the one
//! layout openKRX writes, which is documented and **unverified against any
//! real producer**, and `verified` is `false` in this envelope exactly as it
//! is in every other: openKRX performs no cryptography.

use openkrx_core::StructureReport;
use openkrx_core::profile::CheckOutcome;
use serde::Serialize;

use super::unresolved_rules;

/// The one layout openKRX writes; see `docs/profile.md` for what it is not.
pub const LAYOUT: &str = "canonical-documented";

/// What `create` reports when a package was written and read back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CreateData {
    /// Length of the package, in bytes.
    pub bytes_written: u64,
    /// Archive entries in it: the marker, the metadata document, and one per
    /// attachment.
    pub entries: u32,
    /// Each rule the self-check left undecided, once, in citation order.
    pub unresolved_rules: Vec<&'static str>,
    /// The layout that was written. One exists, and it is unverified.
    pub layout: &'static str,
}

/// Build the report from the written bytes and the self-check's own report.
#[must_use]
pub fn run(bytes_written: u64, entries: u32, report: &StructureReport) -> CreateData {
    CreateData {
        bytes_written,
        entries,
        unresolved_rules: unresolved_rules(report),
        layout: LAYOUT,
    }
}

/// Whether the self-check found a structural check that did not hold.
///
/// A package openKRX wrote must fail none: a failure here is a defect in
/// openKRX, not in the manifest, and the command removes what it wrote rather
/// than handing back a package its own reader rejects.
#[must_use]
pub fn failed(report: &StructureReport) -> bool {
    report
        .checks()
        .iter()
        .any(|check| matches!(check.outcome, CheckOutcome::Fail(_)))
}
