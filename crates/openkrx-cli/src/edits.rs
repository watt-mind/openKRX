//! The edits document `repack` changes a package with, and its validation.
//!
//! An edits document is one JSON object saying what to change: the `FEJRESZ`
//! fields to set, the attachments to add, the ones to replace, and the ones to
//! remove. It is the whole input to `repack` apart from the package and the
//! files it names, and it is validated exactly as `create`'s manifest is, by
//! [`crate::json`]: a key the schema does not define is refused rather than
//! ignored, and every refusal names the field and never the value.
//!
//! **The field spelling is the manifest's.** `metadata` carries the same keys
//! as `create`'s manifest and the diagnostics name the same JSON Pointers,
//! because the two documents describe the same document elements and two
//! spellings would be two schemas. `dispatches` is the one key that is absent:
//! `MELLEKLETEK_SZAMA` and every `MELLEKLET` reference are derived from the
//! attachments the result actually carries.
//!
//! **Absent and `null` are different here**, and only for the four optional
//! header elements. An absent key leaves the element as the package has it;
//! `null` removes it. `create`'s manifest describes a whole document, where
//! the two mean the same thing; an edits document describes a change, where
//! "leave it alone" and "take it out" are two different edits.
//!
//! Nothing here reads a file or resolves a path: the paths are carried as
//! written and resolved by `crate::repack` against the edits document's own
//! directory.
//!
//! The schema is documented in `docs/architecture.md#repacking-a-package`.

use openkrx_core::create::FixedTimestamp;
use openkrx_core::metadata::{ConsignmentKind, SourceSystem};
use openkrx_core::repack::{HeaderEdits, OptionalEdit};
use serde_json::{Map, Value};

use crate::exit::{Failure, MANIFEST_TYPE};
use crate::json;
use crate::manifest::{METADATA_KEYS, field as metadata_field};

/// The keys of the edits object.
const EDITS_KEYS: [&str; 6] = [
    "schema_version",
    "timestamp",
    "metadata",
    "add",
    "replace",
    "remove",
];
/// The keys of one `add` element.
const ADD_KEYS: [&str; 3] = ["path", "file_name", "description"];
/// The keys of one `replace` element.
const REPLACE_KEYS: [&str; 2] = ["number", "path"];
/// The one `metadata` key of the create manifest an edits document refuses:
/// the references and the count are derived, never set.
const DERIVED_KEY: &str = "dispatches";

/// The schema path of every field an edits diagnostic may name.
///
/// The metadata paths are the manifest's own, imported rather than repeated.
pub mod field {
    /// The edits object itself.
    pub const ROOT: &str = "/";
    /// `schema_version`.
    pub const SCHEMA_VERSION: &str = "/schema_version";
    /// `timestamp`.
    pub const TIMESTAMP: &str = "/timestamp";
    /// `metadata`.
    pub const METADATA: &str = "/metadata";
    /// `add`.
    pub const ADD: &str = "/add";
    /// One added attachment's `path`.
    pub const ADD_PATH: &str = "/add/path";
    /// One added attachment's `file_name`.
    pub const ADD_FILE_NAME: &str = "/add/file_name";
    /// One added attachment's `description`.
    pub const ADD_DESCRIPTION: &str = "/add/description";
    /// `replace`.
    pub const REPLACE: &str = "/replace";
    /// One replacement's `number`.
    pub const REPLACE_NUMBER: &str = "/replace/number";
    /// One replacement's `path`.
    pub const REPLACE_PATH: &str = "/replace/path";
    /// `remove`.
    pub const REMOVE: &str = "/remove";
}

/// One attachment to add, before any file is opened.
///
/// Every string is package content: `path` is the caller's own filesystem
/// path, and the other two are written into the package. None ever reaches a
/// diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Addition {
    /// The local file to read, resolved against the edits document's
    /// directory. It is never interpreted, matched or globbed.
    pub path: String,
    /// The name inside the package. Absent means the last component of `path`.
    pub file_name: Option<String>,
    /// `MELLEKLET_LEIRASA`, optional: its absence is unresolved rule M11.
    pub description: Option<String>,
}

/// One attachment's bytes to replace, named by its number in the package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
    /// The attachment's `CSATOLMANY_SZAMA`, counted from 1.
    pub number: u32,
    /// The local file whose bytes take its place.
    pub path: String,
}

/// A validated edits document: what to change, and with which timestamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditsDocument {
    /// The `FEJRESZ` fields to set.
    pub header: HeaderEdits,
    /// Attachment numbers to remove.
    pub remove: Vec<u32>,
    /// Attachment bytes to replace.
    pub replace: Vec<Replacement>,
    /// Attachments to append, in document order.
    pub add: Vec<Addition>,
    /// The MS-DOS date and time every record of the result carries.
    pub timestamp: FixedTimestamp,
}

/// Read one edits document, refusing anything the schema does not define.
///
/// # Errors
///
/// The `manifest.invalid.*` codes, exactly as `create`'s manifest reports
/// them: `syntax`, `schema_version`, `unknown_field`, `missing_field`, `type`,
/// `enumeration` and `timestamp`, each naming the field it concerns and no
/// value the document carries.
pub fn parse(bytes: &[u8]) -> Result<EditsDocument, Failure> {
    let root = json::document(bytes, field::ROOT)?;
    json::known_keys(&root, &EDITS_KEYS, field::ROOT, None)?;
    json::schema_version(&root, field::SCHEMA_VERSION)?;
    let timestamp = json::timestamp(
        &json::required_text(&root, "timestamp", field::TIMESTAMP)?,
        field::TIMESTAMP,
    )?;
    Ok(EditsDocument {
        header: header(&root)?,
        remove: remove(&root)?,
        replace: replace(&root)?,
        add: add(&root)?,
        timestamp,
    })
}

/// The `metadata` object: any subset of the manifest's header fields.
fn header(root: &Map<String, Value>) -> Result<HeaderEdits, Failure> {
    let Some(value) = present(root, "metadata") else {
        return Ok(HeaderEdits::default());
    };
    let map = json::object(value, field::METADATA, None)?;
    let allowed: Vec<&str> = METADATA_KEYS
        .into_iter()
        .filter(|key| *key != DERIVED_KEY)
        .collect();
    json::known_keys(map, &allowed, field::METADATA, None)?;
    Ok(HeaderEdits {
        version: set_text(map, "version", metadata_field::VERSION)?,
        source_system: set_token(
            SourceSystem::parse,
            map,
            "source_system",
            metadata_field::SOURCE_SYSTEM,
        )?,
        consignment_id: set_text(map, "consignment_id", metadata_field::CONSIGNMENT_ID)?,
        created_at_text: set_text(map, "created_at", metadata_field::CREATED_AT)?,
        consignment_kind: set_token(
            ConsignmentKind::parse,
            map,
            "consignment_kind",
            metadata_field::CONSIGNMENT_KIND,
        )?,
        test: set_flag(map, "test", metadata_field::TEST)?,
        barcode: optional(map, "barcode", metadata_field::BARCODE)?,
        reference_id: optional(map, "reference_id", metadata_field::REFERENCE_ID)?,
        error_code: optional(map, "error_code", metadata_field::ERROR_CODE)?,
        note: optional(map, "note", metadata_field::NOTE)?,
    })
}

/// A key that is present and not null, or nothing at all.
///
/// `null` is refused for every field but the four optional header elements,
/// where [`optional`] reads it as a removal: nothing else in either document
/// has a state for it to mean.
fn present<'a>(map: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    match map.get(key) {
        None | Some(Value::Null) => None,
        Some(value) => Some(value),
    }
}

/// A required header element the edits set, or leave alone.
fn set_text(
    map: &Map<String, Value>,
    key: &str,
    field: &'static str,
) -> Result<Option<String>, Failure> {
    match map.get(key) {
        None => Ok(None),
        // A required element cannot be removed, so `null` is a type error
        // rather than a silent no-op.
        Some(value) => json::text(value, field, None).map(Some),
    }
}

/// The same, for an element whose value is one of a fixed set of tokens.
fn set_token<T>(
    parse: impl Fn(&str) -> Option<T>,
    map: &Map<String, Value>,
    key: &str,
    field: &'static str,
) -> Result<Option<T>, Failure> {
    match set_text(map, key, field)? {
        None => Ok(None),
        Some(token) => json::enumeration(parse, &token, field).map(Some),
    }
}

/// The same, for `TESZT`.
fn set_flag(
    map: &Map<String, Value>,
    key: &str,
    field: &'static str,
) -> Result<Option<bool>, Failure> {
    match map.get(key) {
        None => Ok(None),
        Some(_) => json::flag(map, key, field).map(Some),
    }
}

/// One of the four optional header elements: leave it, set it, or remove it.
fn optional(
    map: &Map<String, Value>,
    key: &str,
    field: &'static str,
) -> Result<OptionalEdit, Failure> {
    match map.get(key) {
        None => Ok(OptionalEdit::Keep),
        Some(Value::Null) => Ok(OptionalEdit::Clear),
        Some(value) => json::text(value, field, None).map(OptionalEdit::Set),
    }
}

/// The `remove` array: attachment numbers, counted from 1.
fn remove(root: &Map<String, Value>) -> Result<Vec<u32>, Failure> {
    let Some(value) = present(root, "remove") else {
        return Ok(Vec::new());
    };
    json::array(value, field::REMOVE)?
        .iter()
        .map(|item| number(item, field::REMOVE))
        .collect()
}

/// The `replace` array, in document order.
fn replace(root: &Map<String, Value>) -> Result<Vec<Replacement>, Failure> {
    let Some(value) = present(root, "replace") else {
        return Ok(Vec::new());
    };
    let items = json::array(value, field::REPLACE)?;
    let mut replacements = Vec::with_capacity(items.len());
    for (position, item) in items.iter().enumerate() {
        let index = u32::try_from(position).unwrap_or(u32::MAX);
        let map = json::object(item, field::REPLACE, Some(index))?;
        json::known_keys(map, &REPLACE_KEYS, field::REPLACE, Some(index))?;
        replacements.push(Replacement {
            number: number(
                json::required(map, "number", field::REPLACE_NUMBER, Some(index))?,
                field::REPLACE_NUMBER,
            )?,
            path: json::text(
                json::required(map, "path", field::REPLACE_PATH, Some(index))?,
                field::REPLACE_PATH,
                Some(index),
            )?,
        });
    }
    Ok(replacements)
}

/// The `add` array, in the order the attachments are appended.
fn add(root: &Map<String, Value>) -> Result<Vec<Addition>, Failure> {
    let Some(value) = present(root, "add") else {
        return Ok(Vec::new());
    };
    let items = json::array(value, field::ADD)?;
    let mut additions = Vec::with_capacity(items.len());
    for (position, item) in items.iter().enumerate() {
        let index = u32::try_from(position).unwrap_or(u32::MAX);
        let map = json::object(item, field::ADD, Some(index))?;
        json::known_keys(map, &ADD_KEYS, field::ADD, Some(index))?;
        additions.push(Addition {
            path: json::text(
                json::required(map, "path", field::ADD_PATH, Some(index))?,
                field::ADD_PATH,
                Some(index),
            )?,
            file_name: json::optional_at(map, "file_name", field::ADD_FILE_NAME, index)?,
            description: json::optional_at(map, "description", field::ADD_DESCRIPTION, index)?,
        });
    }
    Ok(additions)
}

/// An attachment number: a JSON integer that fits the numbering.
///
/// Whether the package actually carries that attachment is not decided here:
/// `openkrx_core::repack::plan` answers it against the package itself, and
/// reports `repack.invalid.no_such_attachment` when it does not.
fn number(value: &Value, field: &'static str) -> Result<u32, Failure> {
    value
        .as_u64()
        .and_then(|number| u32::try_from(number).ok())
        .ok_or_else(|| Failure::manifest(MANIFEST_TYPE, field))
}
