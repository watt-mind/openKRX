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
    schema_version: u32,
    ok: bool,
    command: &'a str,
    data: &'a T,
    verified: bool,
}

/// A failed response: the envelope around one content-free diagnostic.
#[derive(Serialize)]
struct Failed<'a> {
    schema_version: u32,
    ok: bool,
    command: &'a str,
    error: Diagnostic<'a>,
    /// What the undo pass removed, present for the two commands that write.
    /// `removed` is `0` when nothing had been written, which is every refusal
    /// decided before the first write; for `create` it is `1` when the
    /// half-written package was removed again. Counts, never a path.
    #[serde(skip_serializing_if = "Option::is_none")]
    cleanup: Option<Cleanup>,
    verified: bool,
}

/// The diagnostic itself: a code, its category, where it happened, and numbers.
///
/// `field` and `attachment_index` belong to `create` and say where in the
/// manifest the refusal was decided. `field` is a JSON Pointer into the
/// manifest, such as `/metadata/source_system`; neither carries a value a
/// manifest author wrote, so the object stays as safe to log as every other
/// diagnostic.
#[derive(Serialize)]
struct Diagnostic<'a> {
    code: &'a str,
    category: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    entry_index: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attachment_index: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u64>,
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
            limit: failure.limit,
            observed: failure.observed,
        },
        verified: false,
    };
    serde_json::to_string(&response).expect("serialisable response")
}
