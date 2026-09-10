//! [`Failure`], the value a run that could not produce a report is reported
//! as, and the one sentence each code explains itself with.
//!
//! A failure carries exactly what a diagnostic may carry: a stable code, its
//! category, an entry index, a manifest field, an attachment position and
//! numbers. An input path, an entry name, a metadata value and anything a
//! manifest author wrote — including the spelling of a key the schema does not
//! define — are never reachable from here.

use openkrx_core::repack::RepackError;
use openkrx_core::{ArchiveError, CreateError, PlanError, ProfileError};

use super::{
    CREATE_SELF_CHECK_FAILED, Category, INPUT_CAP_BYTES, INPUT_OVER_LIMIT, INPUT_UNREADABLE,
    REPACK_SELF_CHECK_FAILED,
};

/// What a diagnostic says when the code's category segment is unknown.
///
/// The fallback category is [`Category::Package`], which is the fail-safe
/// reading — never a success — but its explanation would describe a damaged
/// package, which an unclassified code is no evidence of.
const UNCLASSIFIED_EXPLANATION: &str = "the package was refused with a code this build does not classify; see docs/codes.md for what it means";

/// The sentence one particular code deserves instead of its category's.
///
/// Eight codes share [`Category::Output`], and the one sentence that covers
/// all of them tells a caller with a missing destination about overwriting,
/// which is noise at the moment they need one instruction. Each `output.*`
/// code therefore names its own condition and what to do about it. The seven
/// `manifest.invalid.*` codes are here for the same reason, and because the
/// sentence is where a caller learns what the schema allows: the diagnostic
/// names the field, never its value. Every
/// sentence is content-free: no path, no file name, no operating-system
/// message, so the line stays safe to log wherever the others are.
///
/// **The four codes both writing commands share are keyed on `command` too.**
/// `extract` and `create` reach `output.destination_missing`,
/// `.destination_not_a_directory`, `.destination_symlink` and `.exists` by
/// different routes and are fixed by different actions, and a sentence that
/// covered both would tell a caller who ran one command to do something with
/// the other's flag. Each therefore names the argument the caller actually
/// typed, and nothing else.
fn advice(code: &str, command: &str) -> Option<&'static str> {
    output_advice(code, command)
        .or_else(|| document_advice(code))
        .or_else(|| repack_advice(code))
}

/// The sentence one `output.*` code deserves, keyed on the command too.
///
/// `create` and `repack` both write one package to `--out`, and `extract`
/// writes many files into `--into`. A sentence that covered both would tell a
/// caller who ran one to fix the other one's argument.
fn output_advice(code: &str, command: &str) -> Option<&'static str> {
    let writing = matches!(command, "create" | "repack");
    Some(match code.as_bytes() {
        b"output.destination_missing" if writing => {
            "the directory the --out file would go in does not exist, and \
openkrx never creates one: create it first, or correct the --out argument"
        }
        b"output.destination_missing" => {
            "the destination directory does not exist, and extract never \
creates one: create it first, or correct the --into argument"
        }
        b"output.destination_not_a_directory" if writing => {
            "the --out argument's parent names something that is not a \
directory"
        }
        b"output.destination_not_a_directory" => {
            "the --into argument names something that is not a directory"
        }
        b"output.destination_symlink" if writing => {
            "the directory the --out file would go in is a symbolic link or a \
reparse point; openkrx writes a package only into a real directory, so name \
the --out directory itself"
        }
        b"output.destination_symlink" => {
            "the destination is a symbolic link or a reparse point; extraction \
writes only into a real directory, so name the directory itself"
        }
        b"output.partial_marker_present" => {
            "the destination still holds .openkrx-extract.partial from an \
interrupted run, so what is in it may be incomplete: review it and remove \
that file, or extract into a different directory"
        }
        b"output.exists" if command == "repack" => {
            "something is already at the path --out names. Nothing is ever \
overwritten and no package is edited in place, so --out must name a file that \
does not exist yet — the package being repacked is not it; the edited package \
is written beside it, and the original is left as it was"
        }
        b"output.exists" if writing => {
            "something is already at the path --out names; nothing is ever \
overwritten, so name a file that does not exist yet, or move what is there \
out of the way first"
        }
        b"output.exists" => {
            "a file this package would create is already in the destination; \
nothing is ever overwritten, so extract into an empty directory, or move the \
existing file out of the way first"
        }
        b"output.symlink_in_path" => {
            "a directory this package would write through is a symbolic link \
or a reparse point, which could place output outside the destination"
        }
        b"output.not_a_directory" => {
            "a path this package needs as a directory is something else in the \
destination already"
        }
        b"output.io" => {
            "a create, write or remove failed: check that the destination is \
writable and has free space"
        }
        _ => return None,
    })
}

/// The sentence one `manifest.invalid.*` code deserves.
///
/// Both documents openKRX reads report through these codes, and the sentence
/// is where a caller learns what the schema allows: the diagnostic names the
/// field, never its value.
fn document_advice(code: &str) -> Option<&'static str> {
    Some(match code.as_bytes() {
        b"manifest.invalid.syntax" => {
            "the manifest is not one JSON object: it must be UTF-8, must parse \
as a single object, and must carry nothing after it"
        }
        b"manifest.invalid.schema_version" => {
            "the manifest declares a schema_version this build does not \
implement; 1 is the only one it writes from"
        }
        b"manifest.invalid.unknown_field" => {
            "the manifest carries a key the schema does not define; the \
diagnostic names the object it was in and never the key itself, so compare \
that object against the schema in docs/architecture.md"
        }
        b"manifest.invalid.missing_field" => {
            "a required manifest field is absent; the diagnostic names it"
        }
        b"manifest.invalid.type" => {
            "a manifest field carries the wrong kind of JSON value; the \
diagnostic names the field, and the schema in docs/architecture.md gives its \
type"
        }
        b"manifest.invalid.enumeration" => {
            "a manifest field carries a token outside its fixed set: \
source_system is NOVA, KIR3, KER, POSTA or IMAP, and consignment_kind is \
KULDEMENY, NYUGTA, EXPEDIALAS, TERTIVEVENY or HIBAJELZES"
        }
        b"manifest.invalid.timestamp" => {
            "timestamp must read YYYY-MM-DDTHH:MM:SS and name a time a ZIP \
record can hold: 1980-01-01T00:00:00 to 2107-12-31T23:59:58, with an odd \
second rounded down"
        }
        _ => return None,
    })
}

/// The sentence one `repack.*` code deserves.
///
/// Every one of them says what repacking would have changed or dropped, which
/// is the fact a caller needs: `repack.unsupported.*` is a statement about
/// what openKRX can write, and never about the package being damaged.
fn repack_advice(code: &str) -> Option<&'static str> {
    Some(match code.as_bytes() {
        b"repack.unsupported.root_prefix" => {
            "the package is not in the layout openkrx writes: its metadata \
document sits under a different root prefix, which rule A19 leaves open. \
Repacking would move every entry of the package, so openkrx refuses rather \
than relaying it out; the package is not damaged, and inspect, list, \
validate-structure and extract all read it"
        }
        b"repack.unsupported.metadata_name" => {
            "the metadata document is spelled with a casing rule M12 leaves \
open, and repacking would rename its entry; the package is not damaged and \
every reading command handles it"
        }
        b"repack.unsupported.metadata_missing" => {
            "the package holds no single entry shaped like a metadata \
document, so there is no document to edit; inspect says what it does hold"
        }
        b"repack.unsupported.marker" => {
            "the package's first entry is not the format marker the documented \
layout puts there, so repacking would move or rewrite it; the package is not \
damaged and every reading command handles it"
        }
        b"repack.unsupported.extra_entry" => {
            "the package carries an entry the documented layout has no place \
for — a signature file or a service-specific document, for example — and \
openkrx writes only the marker, the metadata document and the attachments, so \
repacking would drop it. Extract the package instead and keep that file"
        }
        b"repack.unsupported.unknown_elements" => {
            "the metadata document carries elements outside the schema, which \
openkrx counts but does not retain, so repacking would drop them"
        }
        b"repack.unsupported.opaque_block" => {
            "the metadata document carries a block openkrx records the \
presence of but never reads — ERKEZTETES, BONTASOK, TERTIVEVENY or an \
unqualified KEZELESI_UTASITASOK — so repacking would write it back empty"
        }
        b"repack.unsupported.dispatch_count" => {
            "the metadata document carries more than one dispatch block, so \
there is no single place for the derived attachment references"
        }
        b"repack.unsupported.attachment_reference" => {
            "an attachment reference, or the declared attachment count, is not \
what openkrx derives from the attachments themselves — a different size unit \
or number, for instance — so repacking would rewrite it"
        }
        b"repack.unsupported.attachment_entry" => {
            "the payload entries and the attachment references the document \
lists do not describe each other, so repacking would have to decide which of \
the two is right"
        }
        b"repack.invalid.no_such_attachment" => {
            "an edit names an attachment number the package does not carry; \
numbers are the document's own CSATOLMANY_SZAMA, counted from 1, and inspect \
lists them"
        }
        b"repack.invalid.duplicate_target" => {
            "two edits name the same attachment number, so what should happen \
to it is not decided by the edits; name it once"
        }
        b"repack.invalid.inventory_mismatch" => {
            "the plan was applied to a package other than the one it was made \
from, which is a defect in openkrx rather than in your edits"
        }
        b"repack.internal.self_check_failed" => {
            "the package openkrx wrote did not read back cleanly, which is a \
defect in openkrx rather than in your edits; nothing was left behind, and the \
run is worth reporting as a bug"
        }
        b"create.internal.self_check_failed" => {
            "the package openkrx wrote did not read back cleanly, which is a \
defect in openkrx rather than in your manifest; nothing was left behind, and \
the run is worth reporting as a bug"
        }
        _ => return None,
    })
}

/// A run that ended before a report could be produced.
///
/// The fields are exactly what a diagnostic may carry: a stable code, its
/// category, an entry index, a manifest field name, an attachment position and
/// numbers. An input path, an entry name and a metadata value are never
/// reachable from here, and neither is a manifest *value*: [`Failure::field`]
/// is one of a fixed set of schema-defined names, never anything a manifest
/// author wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Failure {
    /// The stable dotted code.
    pub code: &'static str,
    /// The category the code classifies to.
    pub category: Category,
    /// Central-directory index of the entry the failure concerns, if any.
    pub entry_index: Option<u32>,
    /// The manifest field the failure concerns, as a JSON Pointer into the
    /// manifest: `/metadata/source_system`. It is always one of a fixed set of
    /// schema paths, never a key or a value a caller wrote.
    pub field: Option<&'static str>,
    /// Position in the manifest's `attachments` array, or in an edits
    /// document's `add` or `replace` array, when the failure concerns one. It
    /// is not a central-directory index: for a refused request, no archive
    /// exists.
    pub attachment_index: Option<u32>,
    /// The attachment's number inside a package the failure concerns, when it
    /// concerns one: the document's own `CSATOLMANY_SZAMA`, counted from 1.
    /// `repack` alone sets it, and it is a position rather than content.
    pub attachment_number: Option<u32>,
    /// The configured ceiling, when the failure is a limit.
    pub limit: Option<u64>,
    /// The value that reached the ceiling, when it is known.
    pub observed: Option<u64>,
    /// Whether the code's category segment is one this build knows.
    ///
    /// An unclassified code still fails, as [`Category::Package`], because a
    /// code openKRX cannot read must never become a success. The flag only
    /// keeps the human explanation honest about which of the two happened.
    pub classified: bool,
}

impl Failure {
    /// Classify `code`, treating an unknown shape as a package problem.
    pub(super) fn new(code: &'static str) -> Self {
        let category = Category::of_code(code);
        Self {
            code,
            category: category.unwrap_or(Category::Package),
            entry_index: None,
            field: None,
            attachment_index: None,
            attachment_number: None,
            limit: None,
            observed: None,
            classified: category.is_some(),
        }
    }

    /// A manifest the writer cannot be asked to write, scoped to one field.
    ///
    /// `field` is one of a fixed set of schema paths, so a diagnostic that
    /// carries it stays safe to log: the manifest's own values never reach it.
    #[must_use]
    pub fn manifest(code: &'static str, field: &'static str) -> Self {
        Self {
            field: Some(field),
            ..Self::new(code)
        }
    }

    /// The same, scoped to one element of the manifest's `attachments` array.
    #[must_use]
    pub fn manifest_at(code: &'static str, field: &'static str, attachment: u32) -> Self {
        Self {
            attachment_index: Some(attachment),
            ..Self::manifest(code, field)
        }
    }

    /// A package `create` wrote that its own reader did not accept.
    #[must_use]
    pub fn self_check_failed() -> Self {
        Self::new(CREATE_SELF_CHECK_FAILED)
    }

    /// The same, for `repack`: the sentence names the edits rather than a
    /// manifest, because a caller who ran `repack` wrote no manifest.
    #[must_use]
    pub fn repack_self_check_failed() -> Self {
        Self::new(REPACK_SELF_CHECK_FAILED)
    }

    /// The input could not be opened or read.
    #[must_use]
    pub fn unreadable() -> Self {
        Self::new(INPUT_UNREADABLE)
    }

    /// A destination or write failure, carrying no path and no reason.
    #[must_use]
    pub fn output(code: &'static str) -> Self {
        Self::new(code)
    }

    /// The same, scoped to the entry whose output the failure concerns.
    #[must_use]
    pub fn output_at(code: &'static str, entry: u32) -> Self {
        Self {
            entry_index: Some(entry),
            ..Self::new(code)
        }
    }

    /// The input is larger than the cap; `observed` is the cap itself, because
    /// the reader stops there and never learns the real length.
    #[must_use]
    pub fn over_input_cap() -> Self {
        Self {
            limit: Some(openkrx_core::Limits::DEFAULT.max_archive_bytes),
            observed: Some(INPUT_CAP_BYTES),
            ..Self::new(INPUT_OVER_LIMIT)
        }
    }

    /// The message a human-mode diagnostic prints: code, numbers, and where it
    /// happened — an entry index, a manifest field, an attachment position.
    ///
    /// A manifest field is named because the alternative is a caller comparing
    /// a whole document against a schema by hand. Only the schema's own names
    /// are printed: what the manifest says is never part of a diagnostic.
    #[must_use]
    pub fn message(&self) -> String {
        let mut text = self.code.to_owned();
        if let Some(limit) = self.limit {
            text.push_str(&format!(" (limit {limit}"));
            if let Some(observed) = self.observed {
                text.push_str(&format!(", observed {observed}"));
            }
            text.push(')');
        }
        if let Some(entry) = self.entry_index {
            text.push_str(&format!(" at entry {entry}"));
        }
        if let Some(field) = self.field {
            text.push_str(&format!(" at field {field}"));
        }
        if let Some(attachment) = self.attachment_index {
            text.push_str(&format!(" (attachment {attachment})"));
        }
        if let Some(number) = self.attachment_number {
            text.push_str(&format!(" at attachment number {number}"));
        }
        text
    }

    /// The one line human mode writes on stderr: the code, its numbers, and
    /// one sentence saying what the category means. It carries no part of the
    /// input, so it stays safe to log.
    ///
    /// `command` is the name the envelope carries, and it selects between the
    /// two sentences the codes `extract` and `create` share: a caller who ran
    /// one of them must not be told to fix the other one's argument.
    #[must_use]
    pub fn line(&self, command: &str) -> String {
        let explanation = if self.classified {
            advice(self.code, command).unwrap_or_else(|| self.category.explanation())
        } else {
            UNCLASSIFIED_EXPLANATION
        };
        format!(
            "openkrx: {} — {explanation} (exit {})",
            self.message(),
            self.category.status()
        )
    }
}

impl From<ArchiveError> for Failure {
    fn from(error: ArchiveError) -> Self {
        let (limit, observed) = match error {
            ArchiveError::OverLimit {
                limit_value,
                observed,
                ..
            } => (Some(limit_value), observed),
            ArchiveError::Unsupported { value, .. } => (None, value),
            _ => (None, None),
        };
        Self {
            entry_index: error.entry_index(),
            limit,
            observed,
            ..Self::new(error.code())
        }
    }
}

impl From<PlanError> for Failure {
    fn from(error: PlanError) -> Self {
        let (limit, observed) = match error {
            PlanError::OverLimit {
                limit_value,
                observed,
                ..
            } => (Some(limit_value), observed),
            _ => (None, None),
        };
        Self {
            entry_index: error.entry_index(),
            limit,
            observed,
            ..Self::new(error.code())
        }
    }
}

impl From<CreateError> for Failure {
    /// A refused writing request, keeping the attachment it concerns.
    ///
    /// The index is a position in the manifest's `attachments` array, which is
    /// what a caller can act on: nothing was written, so there is no entry to
    /// point at. A ceiling keeps its numbers; nothing else is carried.
    fn from(error: CreateError) -> Self {
        let (limit, observed) = match error {
            CreateError::OverLimit {
                limit_value,
                observed,
                ..
            } => (Some(limit_value), observed),
            _ => (None, None),
        };
        Self {
            attachment_index: error.attachment_index(),
            limit,
            observed,
            ..Self::new(error.code())
        }
    }
}

impl From<ProfileError> for Failure {
    fn from(error: ProfileError) -> Self {
        match error {
            ProfileError::Archive(archive) => Self::from(archive),
            // The enum is `#[non_exhaustive]`; a future variant still carries
            // a stable code, which is all a diagnostic is allowed to report.
            _ => Self::new(error.code()),
        }
    }
}

impl From<RepackError> for Failure {
    /// A refused repacking, keeping the position it concerns.
    ///
    /// The three errors repacking passes through — reading an entry, parsing
    /// the document, writing the result — keep their own codes and their own
    /// numbers, so a consumer buckets them exactly as it does everywhere else.
    /// A refusal of its own carries the central-directory index of the entry,
    /// or the attachment number, that it concerns, and nothing else: the edits
    /// a caller wrote are never part of a diagnostic.
    fn from(error: RepackError) -> Self {
        let inner = match error {
            RepackError::Read(archive) => Some(Self::from(archive)),
            RepackError::Write(create) => Some(Self::from(create)),
            RepackError::Document(_)
            | RepackError::Unsupported { .. }
            | RepackError::Invalid { .. } => None,
            // The enum is `#[non_exhaustive]`; an unknown variant still
            // carries a stable code, which is all a diagnostic may report.
            _ => None,
        };
        let base = inner.unwrap_or_else(|| Self::new(error.code()));
        Self {
            entry_index: error.entry_index().or(base.entry_index),
            attachment_number: error.attachment_number(),
            ..base
        }
    }
}
