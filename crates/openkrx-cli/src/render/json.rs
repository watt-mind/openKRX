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
    verified: bool,
}

/// The diagnostic itself: a code, its category, an index and numbers.
#[derive(Serialize)]
struct Diagnostic<'a> {
    code: &'a str,
    category: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    entry_index: Option<u32>,
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
    let response = Failed {
        schema_version: SCHEMA_VERSION,
        ok: false,
        command,
        error: Diagnostic {
            code: failure.code,
            category: failure.category.as_str(),
            entry_index: failure.entry_index,
            limit: failure.limit,
            observed: failure.observed,
        },
        verified: false,
    };
    serde_json::to_string(&response).expect("serialisable response")
}
