//! Terminal-safe text output.
//!
//! An entry name and a metadata value are attacker-controlled bytes, so
//! nothing reaches a terminal unescaped. A C0 or C1 control, `DEL`, the
//! byte-order mark and the invisible and bidirectional formatting characters
//! are written as `\u{..}`; a byte that is not part of valid UTF-8 is written
//! as `\x{..}`, because rule A21 leaves entry-name encoding unresolved and
//! guessing an encoding would be an invention. A name longer than
//! [`MAX_UNITS`] characters is cut, and the number of characters dropped is
//! stated rather than hidden.
//!
//! Output is bounded without any further rule: the inventory refuses an
//! archive with more than `Limits::DEFAULT.max_entries` (256) entries or a
//! name longer than `max_name_bytes`, so a listing has a bounded number of
//! bounded lines.

use openkrx_core::Capabilities;

use crate::commands::inspect::{AttachmentView, InspectData, MetadataView};
use crate::commands::list::ListData;
use crate::commands::validate::ValidateData;
use crate::commands::{CheckView, OutcomeView};

/// Characters of a name written before the rest is summarised away.
pub const MAX_UNITS: usize = 200;

/// The boundary sentence every report ends with.
const BOUNDARY: &str = "Structural checking is not signature verification and not delivery evidence. \
Nothing is verified.";

/// One decoded unit of a name: a character, or a byte that decoded to none.
enum Unit {
    /// A character of a valid UTF-8 sequence.
    Char(char),
    /// A byte that belongs to no valid UTF-8 sequence.
    Byte(u8),
}

/// Render bytes so that no control sequence and no invisible character can
/// reach a terminal, cutting the result at [`MAX_UNITS`] characters.
#[must_use]
pub fn escape(bytes: &[u8]) -> String {
    let units = decode(bytes);
    let dropped = units.len().saturating_sub(MAX_UNITS);
    let mut text = String::new();
    for unit in units.iter().take(MAX_UNITS) {
        match *unit {
            Unit::Char(value) if hidden(value) => {
                text.push_str(&format!("\\u{{{:04x}}}", u32::from(value)));
            }
            Unit::Char(value) => text.push(value),
            Unit::Byte(value) => text.push_str(&format!("\\x{{{value:02x}}}")),
        }
    }
    if dropped > 0 {
        text.push_str(&format!("…[+{dropped}]"));
    }
    text
}

/// Split bytes into characters, keeping every undecodable byte as itself.
fn decode(bytes: &[u8]) -> Vec<Unit> {
    let mut units = Vec::new();
    let mut rest = bytes;
    loop {
        match core::str::from_utf8(rest) {
            Ok(text) => {
                units.extend(text.chars().map(Unit::Char));
                return units;
            }
            Err(error) => {
                let valid = error.valid_up_to();
                if let Ok(text) = core::str::from_utf8(&rest[..valid]) {
                    units.extend(text.chars().map(Unit::Char));
                }
                let bad = error.error_len().unwrap_or(rest.len() - valid).max(1);
                let end = (valid + bad).min(rest.len());
                units.extend(rest[valid..end].iter().copied().map(Unit::Byte));
                rest = &rest[end..];
            }
        }
    }
}

/// Whether a character must never be written to a terminal as itself.
fn hidden(value: char) -> bool {
    value.is_control()
        || matches!(
            u32::from(value),
            0x200b..=0x200f
                | 0x202a..=0x202e
                | 0x2060..=0x2064
                | 0x2066..=0x2069
                | 0xfeff
                | 0xfff9..=0xfffb
        )
}

/// The capability report: the stage, the operations, and the boundary.
#[must_use]
pub fn capabilities(capabilities: &Capabilities) -> String {
    let operations = if capabilities.operations.is_empty() {
        "none implemented".to_owned()
    } else {
        capabilities.operations.join(", ")
    };
    format!(
        "{}: {}\nOperations: {operations}\n{BOUNDARY}",
        capabilities.project, capabilities.stage
    )
}

/// The entry listing, one aligned row per entry.
#[must_use]
pub fn list(data: &ListData) -> String {
    let mut lines = vec![format!(
        "{:>5}  {:<7}  {:>10}  {:>10}  {:>10}  {:<8}  {}",
        "index", "method", "compressed", "declared", "decoded", "crc32", "name"
    )];
    for entry in &data.entries {
        let name = escape(&entry.name_bytes);
        let note = if entry.name.is_none() {
            "  [name is not UTF-8]"
        } else {
            ""
        };
        lines.push(format!(
            "{:>5}  {:<7}  {:>10}  {:>10}  {:>10}  {:<8}  {name}{note}",
            entry.index,
            entry.method,
            entry.compressed_size,
            entry.declared_size,
            entry.decoded_size,
            entry.crc32,
        ));
    }
    lines.push(String::new());
    lines.push(format!("{} entries.", data.entries.len()));
    lines.push(BOUNDARY.to_owned());
    lines.join("\n")
}

/// The inspection report: observations, declared metadata, then the checks.
#[must_use]
pub fn inspect(data: &InspectData) -> String {
    let observed = &data.observations;
    let mut lines = vec![
        "Package observations".to_owned(),
        row("entries", &observed.entry_count.to_string()),
        row(
            "root prefix",
            &root_prefix(observed.root_prefix_bytes.as_deref()),
        ),
        row(
            "metadata entry",
            &metadata_entry(
                observed.metadata_entry_name_bytes.as_deref(),
                observed.metadata_entry_index,
            ),
        ),
        row("format marker", &outcome(&observed.marker)),
        String::new(),
    ];
    match &data.metadata {
        Some(metadata) => lines.extend(declared(metadata)),
        None => lines.push("No metadata document was parsed; the checks below say why.".to_owned()),
    }
    lines.push(String::new());
    lines.extend(check_lines(&data.checks));
    lines.push(String::new());
    lines.push(BOUNDARY.to_owned());
    lines.join("\n")
}

/// The declared block, stated as declarations rather than as facts.
fn declared(metadata: &MetadataView) -> Vec<String> {
    let mut lines = vec![
        "Declared metadata, as the package writes it; nothing is verified".to_owned(),
        row("KRX version", &escape(metadata.version.as_bytes())),
        row("source system", metadata.source_system),
        row("consignment type", metadata.consignment_type),
        row(
            "consignment id",
            &escape(metadata.consignment_id.as_bytes()),
        ),
    ];
    for (label, value) in [
        ("reference id", &metadata.reference_id),
        ("barcode", &metadata.barcode),
        ("error code", &metadata.error_code),
    ] {
        if let Some(value) = value {
            lines.push(row(label, &escape(value.as_bytes())));
        }
    }
    lines.push(row("created at", &escape(metadata.created_at.as_bytes())));
    lines.push(row(
        "test flag",
        &if metadata.test_present {
            metadata.test.to_string()
        } else {
            "absent (rule M11)".to_owned()
        },
    ));
    if let Some(count) = metadata.declared_attachment_count {
        lines.push(row("attachments declared", &count.to_string()));
    }
    for attachment in &metadata.attachments {
        lines.extend(attachment_lines(attachment));
    }
    lines
}

/// One attachment: what it declares, and what the archive holds for it.
fn attachment_lines(attachment: &AttachmentView) -> Vec<String> {
    let mut lines = vec![
        String::new(),
        row(
            &format!("attachment {}", attachment.number),
            &escape(attachment.declared_path.as_bytes()),
        ),
        row(
            "  declared size",
            &format!(
                "{} (unit unresolved, rule M13)",
                escape(attachment.declared_size_text.as_bytes())
            ),
        ),
    ];
    let resolution = match (attachment.resolution, attachment.entry_index) {
        ("resolved", Some(entry)) => format!("entry {entry}{}", decoded(attachment)),
        ("prefix_variant", Some(entry)) => format!(
            "entry {entry}{}, matched only after adjusting the root prefix \
(rule M14)",
            decoded(attachment)
        ),
        _ => "no entry of this archive has that name".to_owned(),
    };
    lines.push(row("  resolves to", &resolution));
    lines
}

/// The observed decoded size of a resolved entry, as a phrase.
fn decoded(attachment: &AttachmentView) -> String {
    attachment
        .observed_size
        .map_or_else(String::new, |size| format!(", {size} bytes decoded"))
}

/// The structural report: the summary, the checks, the unresolved rules.
#[must_use]
pub fn validate(data: &ValidateData) -> String {
    let mut lines = vec![
        format!("Structural summary: {}", data.summary),
        summary_sentence(data),
        String::new(),
    ];
    lines.extend(check_lines(&data.checks));
    lines.push(String::new());
    if data.unresolved_rules.is_empty() {
        lines.push("No rule was left undecided.".to_owned());
    } else {
        lines.push(format!(
            "Undecided rules: {} — no primary source settles them, so no \
conformance claim is available. See docs/profile.md.",
            data.unresolved_rules.join(", ")
        ));
    }
    lines.push(BOUNDARY.to_owned());
    lines.join("\n")
}

/// One sentence saying what the summary word counts, and what it does not.
fn summary_sentence(data: &ValidateData) -> String {
    let failed = data
        .checks
        .iter()
        .filter(|check| check.outcome == "fail")
        .count();
    let open = data
        .checks
        .iter()
        .filter(|check| check.outcome == "unresolved")
        .count();
    format!(
        "{failed} of {} checks failed and {open} could not be decided. \
This is not a statement that the package is a valid or conforming KRX file.",
        data.checks.len()
    )
}

/// One aligned line per check, with its code or its undecided rule.
fn check_lines(checks: &[CheckView]) -> Vec<String> {
    let mut lines = vec!["Structural checks".to_owned()];
    for check in checks {
        let detail = match (check.code, check.rule) {
            (Some(code), _) => format!("fail ({code})"),
            (None, Some(rule)) => format!("undecided (rule {rule})"),
            (None, None) => check.outcome.replace('_', " "),
        };
        lines.push(format!("  {:<28}{detail}", check.check));
    }
    lines
}

/// One outcome as a phrase, for the marker fact.
fn outcome(view: &OutcomeView) -> String {
    match (view.code, view.rule) {
        (Some(code), _) => format!("fail ({code})"),
        (None, Some(rule)) => format!("undecided (rule {rule})"),
        (None, None) => view.outcome.replace('_', " "),
    }
}

/// A `label  value` line, aligned with the others in its block.
fn row(label: &str, value: &str) -> String {
    format!("  {label:<22}{value}")
}

/// An observed root prefix, which may legitimately be empty.
///
/// One of the three layouts rule A19 describes puts `Metalayer/` at the
/// archive root, so an empty prefix is an observation rather than a missing
/// value, and printing nothing after the label would read as a glitch.
fn root_prefix(bytes: Option<&[u8]>) -> String {
    match bytes {
        None => "not observed".to_owned(),
        Some([]) => "none; the metadata entry is at the archive root".to_owned(),
        Some(bytes) => escape(bytes),
    }
}

/// The metadata entry's name and index, when one was located.
fn metadata_entry(bytes: Option<&[u8]>, index: Option<u32>) -> String {
    match (bytes, index) {
        (Some(name), Some(index)) => format!("{} (entry {index})", escape(name)),
        (Some(name), None) => escape(name),
        _ => "not located".to_owned(),
    }
}
