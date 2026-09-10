//! The one-object JSON envelope.
//!
//! Both shapes carry `schema_version`, `ok`, `command` and `verified`. A
//! successful response carries `data`; a failed one carries `error`. There is
//! no `valid`, `conforming` or `is_krx` field in either, and `verified` is
//! `false` in both, always: openKRX performs no cryptography, so no output of
//! it can ever mean that something was verified.
//!
//! **`ok` is about the command, not about the package.** It says that the
//! report on stdout is a report rather than a diagnostic, and it stays `true`
//! when a structural check failed, because the failure is in the report. A
//! consumer deciding what to do about a package reads `validate-structure`'s
//! `summary`, or the exit status, never `ok`. Renaming the field would break
//! the envelope, so `schema_version` would have to be raised to do it; the
//! contract is documented instead, in `docs/architecture.md`, in
//! `openkrx --help` and in `openkrx validate-structure --help`.
//!
//! `schema_version` is `1`. Adding a field does not raise it, so a consumer
//! must ignore fields it does not recognise; removing, renaming or redefining
//! one does, and a consumer that reads an unknown `schema_version` must stop
//! rather than guess.

use serde::Serialize;

use crate::exit::Failure;
use crate::extract::cleanup::Cleanup;

/// The envelope version. Adding a field is compatible and does not raise it.
const SCHEMA_VERSION: u32 = 1;

/// A successful response: the envelope around one command's data.
#[derive(Serialize)]
struct Success<'a, T> {
    /// [`SCHEMA_VERSION`], so a consumer can tell whether it knows this shape.
    schema_version: u32,
    /// Always `true` here. It says the payload is a report rather than a
    /// diagnostic; it says nothing about the package the report describes.
    ok: bool,
    /// The command that produced the report, as its stable name: `inspect`,
    /// `list`, `validate-structure`, `extract`, `create`, `repack` or
    /// `capabilities`.
    command: &'a str,
    /// The command's own report. Its shape belongs to that command and is
    /// what a raised `schema_version` would be about.
    data: &'a T,
    /// Always `false`. openKRX performs no cryptography, so no run of it can
    /// report that anything was verified.
    verified: bool,
}

/// A failed response: the envelope around one content-free diagnostic.
#[derive(Serialize)]
struct Failed<'a> {
    /// [`SCHEMA_VERSION`], the same field a successful response carries.
    schema_version: u32,
    /// Always `false` here: stdout carries a diagnostic, not a report. A
    /// structural check that failed does not reach this shape; it is part of
    /// a successful command's report.
    ok: bool,
    /// The command that refused, as its stable name.
    command: &'a str,
    /// The single content-free diagnostic that says why the command refused.
    error: Diagnostic<'a>,
    /// What the undo pass removed, present for the three commands that write.
    /// `removed` is `0` when nothing had been written, which is every refusal
    /// decided before the first write; for `create` it is `1` when the
    /// half-written package was removed again. Counts, never a path.
    #[serde(skip_serializing_if = "Option::is_none")]
    cleanup: Option<Cleanup>,
    /// Always `false`, for the same reason as on a successful response.
    verified: bool,
}

/// The diagnostic itself: a code, its category, where it happened, and numbers.
///
/// `field` and `attachment_index` belong to `create` and `repack` and say
/// where in the manifest, or in the edits document, the refusal was decided.
/// `field` is a JSON Pointer into that document, such as
/// `/metadata/source_system`. `attachment_number` belongs to `repack` alone
/// and is an attachment's number inside the package being edited, counted
/// from 1, which is a different thing from a position in an array. None of
/// them carries a value an author wrote, so the object stays as safe to log
/// as every other diagnostic.
#[derive(Serialize)]
struct Diagnostic<'a> {
    /// The stable dotted code, catalogued section by section in
    /// `docs/codes.md` under "Reading a diagnostic". This is the field a
    /// consumer branches on, and an unrecognised code is a failure, never a
    /// success.
    code: &'a str,
    /// The coarse family the code belongs to, from
    /// [`crate::exit::Category::as_str`]: `usage`, `structure_inconsistent`,
    /// `structure_unresolved`, `input`, `package`, `unsupported`, `limit` or
    /// `output`. It is the same category that decided the process exit
    /// status, so a consumer that groups rather than branches can read either.
    category: &'a str,
    /// The archive entry the refusal is about, counted in central-directory
    /// order. Absent when the refusal is about no single entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    entry_index: Option<u32>,
    /// A JSON Pointer into the manifest, or into the edits document, such as
    /// `/metadata/source_system`. It names a location in a document the
    /// caller wrote, never a value from it. `create` and `repack` only.
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<&'a str>,
    /// The position in the manifest's or the edits document's attachment
    /// array, counted from 0. `create` and `repack` only.
    #[serde(skip_serializing_if = "Option::is_none")]
    attachment_index: Option<u32>,
    /// An attachment's number inside the package being edited, counted from
    /// 1, which is a different thing from a position in an array. `repack`
    /// alone sets it.
    #[serde(skip_serializing_if = "Option::is_none")]
    attachment_number: Option<u32>,
    /// The ceiling that was exceeded, for a limit refusal.
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u64>,
    /// What was actually produced against that ceiling — a measured count of
    /// bytes or entries, never a size a header declared.
    #[serde(skip_serializing_if = "Option::is_none")]
    observed: Option<u64>,
}

/// Serialise one successful response as a single line.
///
/// Every report type is built from strings, numbers, booleans and sequences,
/// so serialisation cannot meet a value `serde_json` refuses; a map with a
/// non-string key, the one such value, exists in none of them.
pub fn success<T: Serialize>(command: &str, data: &T) -> String {
    let response = Success {
        schema_version: SCHEMA_VERSION,
        ok: true,
        command,
        data,
        verified: false,
    };
    serde_json::to_string(&response).expect("serialisable response")
}

/// Serialise one failed response as a single line.
pub fn failure(command: &str, failure: &Failure) -> String {
    failed(command, failure, None)
}

/// The same, carrying what the undo pass removed.
///
/// A caller of `extract` or `create` needs to know whether the destination was
/// left as it was found. `removed` is how many of this run's paths are gone and
/// `left_in_place` how many could not be removed; both are counts, because a
/// diagnostic may never carry a path.
pub fn failure_with_cleanup(command: &str, failure: &Failure, cleanup: Cleanup) -> String {
    failed(command, failure, Some(cleanup))
}

/// Build the failed envelope, with or without the cleanup counts.
fn failed(command: &str, failure: &Failure, cleanup: Option<Cleanup>) -> String {
    let response = Failed {
        schema_version: SCHEMA_VERSION,
        ok: false,
        command,
        cleanup,
        error: Diagnostic {
            code: failure.code,
            category: failure.category.as_str(),
            entry_index: failure.entry_index,
            field: failure.field,
            attachment_index: failure.attachment_index,
            attachment_number: failure.attachment_number,
            limit: failure.limit,
            observed: failure.observed,
        },
        verified: false,
    };
    serde_json::to_string(&response).expect("serialisable response")
}

#[cfg(test)]
mod tests {
    //! The schema and the envelope, held together.
    //!
    //! `docs/schema/openkrx-envelope.v1.schema.json` is the machine-readable
    //! form of this module's contract, and `scripts/check-schema.py` validates
    //! every recorded envelope against it. Neither of those notices a schema
    //! that has stopped describing *this build*: a command renamed here, a
    //! stable-code head added, a category introduced or `schema_version`
    //! raised leaves the goldens and the schema agreeing with one another
    //! while both drift away from the executable. These tests close that gap
    //! by reading the schema and comparing what it names with what the binary
    //! actually has.

    use openkrx_core::capabilities;
    use serde_json::Value;

    use super::SCHEMA_VERSION;
    use crate::exit::Category;

    /// The published schema, read at compile time from the repository.
    const SCHEMA: &str = include_str!("../../../../docs/schema/openkrx-envelope.v1.schema.json");

    /// The stable-code checker, whose `HEADS` line is the one head list.
    const CHECKER: &str = include_str!("../../../../scripts/check-codes.py");

    /// Every category a diagnostic can carry, which is every one but success.
    ///
    /// The match is the point: adding a variant to [`Category`] stops this
    /// file compiling, so a new category cannot reach the envelope without
    /// someone deciding whether the schema should name it.
    const fn is_failure_category(category: Category) -> bool {
        match category {
            Category::Success => false,
            Category::Usage
            | Category::Inconsistent
            | Category::Unresolved
            | Category::Input
            | Category::Package
            | Category::Unsupported
            | Category::Limit
            | Category::Output => true,
        }
    }

    /// Every category this build defines, in exit-status order.
    const CATEGORIES: [Category; 9] = [
        Category::Success,
        Category::Usage,
        Category::Inconsistent,
        Category::Unresolved,
        Category::Input,
        Category::Package,
        Category::Unsupported,
        Category::Limit,
        Category::Output,
    ];

    /// The parsed schema.
    fn schema() -> Value {
        serde_json::from_str(SCHEMA).expect("the published schema must be JSON")
    }

    /// The strings one `$defs` entry's `enum` names, sorted.
    fn definition_enum(name: &str) -> Vec<String> {
        let schema = schema();
        let values = schema["$defs"][name]["enum"]
            .as_array()
            .unwrap_or_else(|| panic!("$defs/{name} must carry an enum"))
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .unwrap_or_else(|| panic!("$defs/{name} enum must be strings"))
                    .to_owned()
            })
            .collect::<Vec<String>>();
        assert!(!values.is_empty(), "$defs/{name} names nothing");
        let mut sorted = values;
        sorted.sort();
        sorted
    }

    /// The heads the schema's code pattern alternates over, sorted.
    ///
    /// The pattern is `^(a|b|…)(\.[a-z0-9_]+)+$`, so the alternation between
    /// the first pair of parentheses is the head list. Reading it back rather
    /// than restating it here is what makes the comparison below a check.
    fn schema_code_heads() -> Vec<String> {
        let schema = schema();
        let pattern = schema["$defs"]["code"]["pattern"]
            .as_str()
            .expect("$defs/code must carry a pattern")
            .to_owned();
        let open = pattern.find('(').expect("the pattern must alternate");
        let close = pattern.find(')').expect("the pattern must alternate");
        assert!(open < close, "the pattern's first group must be the heads");
        let mut heads: Vec<String> = pattern[open + 1..close]
            .split('|')
            .map(str::to_owned)
            .collect();
        assert!(heads.len() > 1, "the pattern names one head: {pattern}");
        heads.sort();
        heads
    }

    /// The heads `scripts/check-codes.py` extracts, from its `HEADS` line.
    fn checker_heads() -> Vec<String> {
        let line = CHECKER
            .lines()
            .find(|line| line.starts_with("HEADS = ("))
            .expect("scripts/check-codes.py must keep its one-line HEADS tuple");
        let mut heads: Vec<String> = line
            .split('"')
            .skip(1)
            .step_by(2)
            .map(str::to_owned)
            .collect();
        assert!(!heads.is_empty(), "the HEADS line names no head: {line}");
        heads.sort();
        heads
    }

    #[test]
    fn the_schema_pins_the_version_this_build_writes() {
        let schema = schema();
        let published = schema["properties"]["schema_version"]["const"]
            .as_u64()
            .expect("the schema must pin schema_version to a constant");
        assert_eq!(
            published,
            u64::from(SCHEMA_VERSION),
            "docs/schema/openkrx-envelope.v1.schema.json pins a schema_version \
this build does not write; the schema moves with the envelope"
        );
        assert_eq!(
            schema["properties"]["verified"]["const"],
            Value::Bool(false),
            "verified is false in every response openKRX writes, so the schema \
must pin it rather than merely type it"
        );
    }

    #[test]
    fn every_command_the_schema_names_exists_in_this_build() {
        // Every command that renders an envelope is a package operation, plus
        // `capabilities`, which reports the operations themselves. `skill`,
        // `completions` and `man` bypass the envelope and belong to neither
        // list.
        let mut expected: Vec<String> = capabilities()
            .operations
            .iter()
            .map(|operation| (*operation).to_owned())
            .collect();
        expected.push("capabilities".to_owned());
        expected.sort();
        assert_eq!(
            definition_enum("command"),
            expected,
            "the commands docs/schema/openkrx-envelope.v1.schema.json names \
and the operations this build implements disagree; a command that writes an \
envelope needs a data shape in the schema"
        );
    }

    #[test]
    fn every_code_head_the_schema_names_exists_in_this_build() {
        assert_eq!(
            schema_code_heads(),
            checker_heads(),
            "the code heads docs/schema/openkrx-envelope.v1.schema.json \
alternates over and the HEADS line of scripts/check-codes.py disagree; a head \
belongs in both, so that a code the catalogue documents also validates"
        );
    }

    #[test]
    fn every_category_the_schema_names_exists_in_this_build() {
        let mut expected: Vec<String> = CATEGORIES
            .iter()
            .filter(|category| is_failure_category(**category))
            .map(|category| category.as_str().to_owned())
            .collect();
        expected.sort();
        assert_eq!(expected.len(), CATEGORIES.len() - 1, "one success category");
        assert_eq!(
            definition_enum("category"),
            expected,
            "the categories docs/schema/openkrx-envelope.v1.schema.json names \
and Category::as_str disagree; a diagnostic carries the category that decided \
the exit status, so the two lists are the same list"
        );
    }
}
