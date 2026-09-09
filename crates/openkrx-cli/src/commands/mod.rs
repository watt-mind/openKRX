//! The three reader commands, each rendering core types and nothing more.
//!
//! A command in this module reads no bytes and decides no package semantics.
//! It converts what `openkrx_core::archive::inventory` and
//! `openkrx_core::profile::check` already produced into one presentation type,
//! which `crate::render` then writes as JSON or as text. Nothing here can
//! introduce a rule the core crate does not hold.

pub mod inspect;
pub mod list;
pub mod validate;

use openkrx_core::profile::{Check, CheckOutcome, StructureReport, StructureSummary};
use serde::Serialize;

use crate::exit::Category;

/// One named structural check and what it concluded.
///
/// The four outcomes stay distinct in every rendering: an unresolved rule is
/// never shown as a pass and never as a failure, because
/// `docs/profile.md` leaves the rule it cites open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CheckView {
    /// The check's stable snake-case identifier.
    pub check: &'static str,
    /// `pass`, `fail`, `unresolved` or `not_applicable`.
    pub outcome: &'static str,
    /// The stable code, when the outcome is a failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<&'static str>,
    /// The unresolved rule of `docs/profile.md`, when the outcome is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<&'static str>,
}

impl From<&Check> for CheckView {
    fn from(check: &Check) -> Self {
        let (outcome, code, rule) = describe(check.outcome);
        Self {
            check: check.id.as_str(),
            outcome,
            code,
            rule,
        }
    }
}

/// One outcome without the check it belongs to, used for the marker fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct OutcomeView {
    /// `pass`, `fail`, `unresolved` or `not_applicable`.
    pub outcome: &'static str,
    /// The stable code, when the outcome is a failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<&'static str>,
    /// The unresolved rule of `docs/profile.md`, when the outcome is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<&'static str>,
}

impl From<CheckOutcome> for OutcomeView {
    fn from(outcome: CheckOutcome) -> Self {
        let (outcome, code, rule) = describe(outcome);
        Self {
            outcome,
            code,
            rule,
        }
    }
}

/// Split an outcome into the three stable strings a renderer needs.
fn describe(outcome: CheckOutcome) -> (&'static str, Option<&'static str>, Option<&'static str>) {
    match outcome {
        CheckOutcome::Pass => ("pass", None, None),
        CheckOutcome::Fail(code) => ("fail", Some(code), None),
        CheckOutcome::Unresolved(rule) => ("unresolved", None, Some(rule.as_str())),
        // The enum is `#[non_exhaustive]`; anything this build does not know
        // is reported as undecided rather than folded into a pass.
        CheckOutcome::NotApplicable => ("not_applicable", None, None),
        _ => ("unresolved", None, None),
    }
}

/// Every check in `CheckId::ORDER`, which is stable across runs and platforms.
fn checks(report: &StructureReport) -> Vec<CheckView> {
    report.checks().iter().map(CheckView::from).collect()
}

/// Each unresolved rule once, in the order the checks first cite it.
fn unresolved_rules(report: &StructureReport) -> Vec<&'static str> {
    let mut rules: Vec<&'static str> = Vec::new();
    for check in report.checks() {
        if let CheckOutcome::Unresolved(rule) = check.outcome
            && !rules.contains(&rule.as_str())
        {
            rules.push(rule.as_str());
        }
    }
    rules
}

/// The summary word, and the exit category `validate-structure` reports it as.
fn summary(report: &StructureReport) -> (&'static str, Category) {
    match report.summary() {
        StructureSummary::Consistent => ("consistent", Category::Success),
        StructureSummary::Inconsistent => ("inconsistent", Category::Inconsistent),
        StructureSummary::Unresolved => ("unresolved", Category::Unresolved),
        // A summary this build does not know is not a success.
        _ => ("unresolved", Category::Unresolved),
    }
}

/// Lower-case hexadecimal of raw bytes, reported when a name is not UTF-8.
///
/// A name the archive spells in some other encoding has no faithful string
/// form, and guessing one would be an invention: rule A21 leaves entry-name
/// encoding unresolved. The bytes are reported as they are instead.
fn hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push_str(&format!("{byte:02x}"));
    }
    text
}

/// The text form of a name, when the bytes are valid UTF-8.
fn text(bytes: &[u8]) -> Option<String> {
    core::str::from_utf8(bytes).ok().map(str::to_owned)
}

/// The hexadecimal form, present only when there is no text form.
fn hex_when_not_utf8(bytes: &[u8]) -> Option<String> {
    core::str::from_utf8(bytes).err().map(|_| hex(bytes))
}
