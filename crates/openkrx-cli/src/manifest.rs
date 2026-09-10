//! The manifest `create` writes a package from, and its strict validation.
//!
//! A manifest is one JSON object describing a package to write: the metadata
//! document's typed values, the local files to carry as attachments, and the
//! timestamp every ZIP record gets. It is the whole input to `create` apart
//! from those files, and it is deliberately not a free-form document.
//!
//! **Validation is strict in both directions**, and [`crate::json`] is what
//! enforces it: a required field that is absent is refused with
//! `manifest.invalid.missing_field`, and a key the schema does not define is
//! refused with `manifest.invalid.unknown_field` rather than ignored. Every
//! refusal names the *field*, from the fixed set of schema paths in this
//! module — never the value, and never the unknown key itself.
//!
//! The field paths and the `metadata` key list are `pub(crate)` because
//! `repack`'s edits document names the same fields with the same spelling, and
//! two lists would be two spellings.
//!
//! Nothing here reads a file, resolves a path or consults a clock. The
//! attachment paths are carried as written and resolved by `crate::create`
//! against the manifest's own directory; the timestamp is converted to the two
//! MS-DOS fields a ZIP record holds, and openKRX never asks the machine what
//! time it is.
//!
//! The schema is documented in `docs/architecture.md#creating-a-package`, and
//! this module is what enforces it.

use openkrx_core::create::FixedTimestamp;
use openkrx_core::draft::{self, HeaderDraft};
use openkrx_core::metadata::{ConsignmentKind, Metadata, SourceSystem};
use serde_json::{Map, Value};

use crate::exit::{Failure, MANIFEST_TYPE};
use crate::json;

/// The keys of the manifest object.
///
/// `pub(crate)` because the sentence `manifest.invalid.unknown_field`
/// explains itself with lists them, and a test holds the two together.
pub(crate) const MANIFEST_KEYS: [&str; 4] =
    ["schema_version", "metadata", "attachments", "timestamp"];
/// The keys of `metadata`.
pub(crate) const METADATA_KEYS: [&str; 11] = [
    "version",
    "source_system",
    "consignment_id",
    "created_at",
    "consignment_kind",
    "test",
    "barcode",
    "reference_id",
    "error_code",
    "note",
    "dispatches",
];
/// The keys of one `metadata.dispatches` element.
pub(crate) const DISPATCH_KEYS: [&str; 1] = ["declared_attachment_count"];
/// The keys of one `attachments` element.
pub(crate) const ATTACHMENT_KEYS: [&str; 3] = ["path", "file_name", "description"];

/// The schema path of every field a diagnostic may name.
///
/// The paths are JSON Pointers into the manifest (RFC 6901), with array
/// indices left out: `attachment_index` on the failure carries the position,
/// and the pointer names the shape. The separator is `/` rather than `.` for
/// a second reason too — a dotted lower-case path is the shape of a stable
/// code, and a field name must never be mistaken for one.
pub(crate) mod field {
    /// The manifest object itself.
    pub const ROOT: &str = "/";
    /// `schema_version`.
    pub const SCHEMA_VERSION: &str = "/schema_version";
    /// `timestamp`.
    pub const TIMESTAMP: &str = "/timestamp";
    /// `metadata`.
    pub const METADATA: &str = "/metadata";
    /// `metadata.version`.
    pub const VERSION: &str = "/metadata/version";
    /// `metadata.source_system`.
    pub const SOURCE_SYSTEM: &str = "/metadata/source_system";
    /// `metadata.consignment_id`.
    pub const CONSIGNMENT_ID: &str = "/metadata/consignment_id";
    /// `metadata.created_at`.
    pub const CREATED_AT: &str = "/metadata/created_at";
    /// `metadata.consignment_kind`.
    pub const CONSIGNMENT_KIND: &str = "/metadata/consignment_kind";
    /// `metadata.test`.
    pub const TEST: &str = "/metadata/test";
    /// `metadata.barcode`.
    pub const BARCODE: &str = "/metadata/barcode";
    /// `metadata.reference_id`.
    pub const REFERENCE_ID: &str = "/metadata/reference_id";
    /// `metadata.error_code`.
    pub const ERROR_CODE: &str = "/metadata/error_code";
    /// `metadata.note`.
    pub const NOTE: &str = "/metadata/note";
    /// `metadata.dispatches`.
    pub const DISPATCHES: &str = "/metadata/dispatches";
    /// One dispatch block's `declared_attachment_count`.
    pub const DECLARED_COUNT: &str = "/metadata/dispatches/declared_attachment_count";
    /// `attachments`.
    pub const ATTACHMENTS: &str = "/attachments";
    /// One attachment's `path`.
    pub const PATH: &str = "/attachments/path";
    /// One attachment's `file_name`.
    pub const FILE_NAME: &str = "/attachments/file_name";
    /// One attachment's `description`.
    pub const DESCRIPTION: &str = "/attachments/description";
}

/// One attachment as the manifest describes it, before any file is opened.
///
/// Both strings are package content: `path` is the caller's own filesystem
/// path and `file_name` is the name inside the package. Neither ever reaches a
/// diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    /// The local file to read, as written, resolved against the manifest's
    /// directory. It is never interpreted, matched or globbed.
    pub path: String,
    /// The name inside the package. Absent means the last component of `path`.
    pub file_name: Option<String>,
    /// `MELLEKLET_LEIRASA`, optional: its absence is unresolved rule M11.
    pub description: Option<String>,
}

/// A validated manifest: what to write, and with which timestamp.
#[derive(Debug, Clone, PartialEq)]
pub struct Manifest {
    /// The typed document, with no derived reference in it yet.
    pub metadata: Metadata,
    /// The attachments, in the order they will be numbered and placed.
    pub attachments: Vec<Attachment>,
    /// The MS-DOS date and time every record carries.
    pub timestamp: FixedTimestamp,
}

/// Read one manifest, refusing anything the schema does not define.
///
/// # Errors
///
/// `manifest.invalid.syntax` when the bytes are not one JSON object,
/// `manifest.invalid.schema_version` for a version this build does not
/// implement, and `manifest.invalid.unknown_field`, `.missing_field`, `.type`,
/// `.enumeration` or `.timestamp` for the field each names. Every failure
/// carries the schema path of the field it concerns, and no value from the
/// manifest.
pub fn parse(bytes: &[u8]) -> Result<Manifest, Failure> {
    let root = json::document(bytes, field::ROOT)?;
    json::known_keys(&root, &MANIFEST_KEYS, field::ROOT, None)?;
    json::schema_version(&root, field::SCHEMA_VERSION)?;

    let attachments = match root.get("attachments") {
        None | Some(Value::Null) => Vec::new(),
        Some(value) => attachments(value)?,
    };
    let metadata_object = json::object(
        json::required(&root, "metadata", field::METADATA, None)?,
        field::METADATA,
        None,
    )?;
    json::known_keys(metadata_object, &METADATA_KEYS, field::METADATA, None)?;
    let metadata = document(metadata_object, attachments.len())?;

    let timestamp = json::timestamp(
        &json::required_text(&root, "timestamp", field::TIMESTAMP)?,
        field::TIMESTAMP,
    )?;
    Ok(Manifest {
        metadata,
        attachments,
        timestamp,
    })
}

/// The typed document, carrying no attachment reference: the writer derives
/// every one of them from the files it is actually given.
fn document(map: &Map<String, Value>, attachment_count: usize) -> Result<Metadata, Failure> {
    let header = HeaderDraft {
        version: json::required_text(map, "version", field::VERSION)?,
        source_system: json::enumeration(
            SourceSystem::parse,
            &json::required_text(map, "source_system", field::SOURCE_SYSTEM)?,
            field::SOURCE_SYSTEM,
        )?,
        consignment_id: json::required_text(map, "consignment_id", field::CONSIGNMENT_ID)?,
        created_at_text: json::required_text(map, "created_at", field::CREATED_AT)?,
        consignment_kind: json::enumeration(
            ConsignmentKind::parse,
            &json::required_text(map, "consignment_kind", field::CONSIGNMENT_KIND)?,
            field::CONSIGNMENT_KIND,
        )?,
        test: json::flag(map, "test", field::TEST)?,
        barcode: json::optional_text(map, "barcode", field::BARCODE)?,
        reference_id: json::optional_text(map, "reference_id", field::REFERENCE_ID)?,
        error_code: json::optional_text(map, "error_code", field::ERROR_CODE)?,
        note: json::optional_text(map, "note", field::NOTE)?,
    };
    Ok(draft::metadata(
        header.build(),
        dispatches(map, attachment_count)?,
    ))
}

/// The `EXPEDIALAS` blocks.
///
/// Absent means "as many as the attachments need": one block when there is an
/// attachment to list in it, none when there is not. The array is accepted so
/// that a manifest can assert `declared_attachment_count`, which the writer
/// then checks against the attachments rather than writing in their place.
fn dispatches(
    map: &Map<String, Value>,
    attachment_count: usize,
) -> Result<Vec<openkrx_core::metadata::Dispatch>, Failure> {
    let derived = || {
        if attachment_count == 0 {
            Vec::new()
        } else {
            vec![draft::dispatch(None)]
        }
    };
    let Some(value) = map.get("dispatches") else {
        return Ok(derived());
    };
    if value.is_null() {
        return Ok(derived());
    }
    let items = json::array(value, field::DISPATCHES)?;
    let mut dispatches = Vec::with_capacity(items.len());
    for item in items {
        let block = json::object(item, field::DISPATCHES, None)?;
        json::known_keys(block, &DISPATCH_KEYS, field::DISPATCHES, None)?;
        let count = match block.get("declared_attachment_count") {
            None | Some(Value::Null) => None,
            Some(value) => Some(
                value
                    .as_i64()
                    .ok_or_else(|| Failure::manifest(MANIFEST_TYPE, field::DECLARED_COUNT))?,
            ),
        };
        dispatches.push(draft::dispatch(count));
    }
    Ok(dispatches)
}

/// The `attachments` array, in the order it is written.
fn attachments(value: &Value) -> Result<Vec<Attachment>, Failure> {
    let items = json::array(value, field::ATTACHMENTS)?;
    let mut attachments = Vec::with_capacity(items.len());
    for (position, item) in items.iter().enumerate() {
        let index = u32::try_from(position).unwrap_or(u32::MAX);
        let map = json::object(item, field::ATTACHMENTS, Some(index))?;
        json::known_keys(map, &ATTACHMENT_KEYS, field::ATTACHMENTS, Some(index))?;
        attachments.push(Attachment {
            path: json::text(
                json::required(map, "path", field::PATH, Some(index))?,
                field::PATH,
                Some(index),
            )?,
            file_name: json::optional_at(map, "file_name", field::FILE_NAME, index)?,
            description: json::optional_at(map, "description", field::DESCRIPTION, index)?,
        });
    }
    Ok(attachments)
}
