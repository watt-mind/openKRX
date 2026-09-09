//! `validate-structure`: the check inventory, outcome by outcome.
//!
//! The command deliberately does not answer "is this a valid KRX package?".
//! No such answer exists: `docs/profile.md` lists essential rules that no
//! primary source settles, so a check that depends on one reports the rule as
//! unresolved. The summary word says only whether anything failed and whether
//! anything was left open, and the exit status encodes exactly that.

use openkrx_core::profile::StructureReport;
use serde::Serialize;

use super::{CheckView, checks, summary, unresolved_rules};
use crate::exit::Category;

/// What `validate-structure` reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidateData {
    /// `consistent`, `inconsistent` or `unresolved`. Never a verdict.
    pub summary: &'static str,
    /// Every structural check, in the documented order.
    pub checks: Vec<CheckView>,
    /// Each unresolved rule once, in the order the checks first cite it.
    pub unresolved_rules: Vec<&'static str>,
}

/// Build the report, and the exit category the summary maps to.
#[must_use]
pub fn run(report: &StructureReport) -> (ValidateData, Category) {
    let (word, category) = summary(report);
    (
        ValidateData {
            summary: word,
            checks: checks(report),
            unresolved_rules: unresolved_rules(report),
        },
        category,
    )
}
