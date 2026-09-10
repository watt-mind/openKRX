//! The manifest `create` writes a package from, and its strict validation.
//!
//! A manifest is one JSON object describing a package to write: the metadata
//! document's typed values, the local files to carry as attachments, and the
//! timestamp every ZIP record gets. It is the whole input to `create` apart
//! from those files, and it is deliberately not a free-form document.
//!
//! **Validation is strict in both directions.** A required field that is
//! absent is refused with `manifest.invalid.missing_field`, and a key the
//! schema does not define is refused with `manifest.invalid.unknown_field`
//! rather than ignored: a manifest whose `atachments` key was silently dropped
//! would produce a package with no attachment and a success report, which is
//! the worst outcome available. Every refusal names the *field*, from the
//! fixed set of schema paths in this module — never the value, and never the
//! unknown key itself, so a diagnostic stays as safe to log as any other.
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

use crate::exit::{
    Failure, MANIFEST_ENUMERATION, MANIFEST_MISSING_FIELD, MANIFEST_SCHEMA_VERSION,
    MANIFEST_SYNTAX, MANIFEST_TIMESTAMP, MANIFEST_TYPE, MANIFEST_UNKNOWN_FIELD,
};

/// The only manifest schema this build writes from.
pub const SCHEMA_VERSION: u64 = 1;

/// The characters of `YYYY-MM-DDTHH:MM:SS`.
const TIMESTAMP_LENGTH: usize = 19;

/// The keys of the manifest object.
const MANIFEST_KEYS: [&str; 4] = ["schema_version", "metadata", "attachments", "timestamp"];
/// The keys of `metadata`.
const METADATA_KEYS: [&str; 11] = [
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
const DISPATCH_KEYS: [&str; 1] = ["declared_attachment_count"];
/// The keys of one `attachments` element.
const ATTACHMENT_KEYS: [&str; 3] = ["path", "file_name", "description"];

/// The schema path of every field a diagnostic may name.
///
/// The paths are JSON Pointers into the manifest (RFC 6901), with array
/// indices left out: `attachment_index` on the failure carries the position,
/// and the pointer names the shape. The separator is `/` rather than `.` for
/// a second reason too — a dotted lower-case path is the shape of a stable
/// code, and a field name must never be mistaken for one.
mod field {
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
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| Failure::manifest(MANIFEST_SYNTAX, field::ROOT))?;
    let root = value
        .as_object()
        .ok_or_else(|| Failure::manifest(MANIFEST_SYNTAX, field::ROOT))?;
    known_keys(root, &MANIFEST_KEYS, field::ROOT, None)?;
    schema_version(root)?;

    let attachments = match root.get("attachments") {
        None | Some(Value::Null) => Vec::new(),
        Some(value) => attachments(value)?,
    };
    let metadata_object = object(
        required(root, "metadata", field::METADATA, None)?,
        field::METADATA,
        None,
    )?;
    known_keys(metadata_object, &METADATA_KEYS, field::METADATA, None)?;
    let metadata = document(metadata_object, attachments.len())?;

    let timestamp = timestamp(text(
        required(root, "timestamp", field::TIMESTAMP, None)?,
        field::TIMESTAMP,
        None,
    )?)?;
    Ok(Manifest {
        metadata,
        attachments,
        timestamp,
    })
}

/// `schema_version` must be exactly the version this build implements.
fn schema_version(root: &Map<String, Value>) -> Result<(), Failure> {
    let value = required(root, "schema_version", field::SCHEMA_VERSION, None)?;
    let declared = value
        .as_u64()
        .ok_or_else(|| Failure::manifest(MANIFEST_TYPE, field::SCHEMA_VERSION))?;
    if declared != SCHEMA_VERSION {
        return Err(Failure::manifest(
            MANIFEST_SCHEMA_VERSION,
            field::SCHEMA_VERSION,
        ));
    }
    Ok(())
}

/// The typed document, carrying no attachment reference: the writer derives
/// every one of them from the files it is actually given.
fn document(map: &Map<String, Value>, attachment_count: usize) -> Result<Metadata, Failure> {
    let header = HeaderDraft {
        version: required_text(map, "version", field::VERSION)?,
        source_system: enumeration(
            SourceSystem::parse,
            &required_text(map, "source_system", field::SOURCE_SYSTEM)?,
            field::SOURCE_SYSTEM,
        )?,
        consignment_id: required_text(map, "consignment_id", field::CONSIGNMENT_ID)?,
        created_at_text: required_text(map, "created_at", field::CREATED_AT)?,
        consignment_kind: enumeration(
            ConsignmentKind::parse,
            &required_text(map, "consignment_kind", field::CONSIGNMENT_KIND)?,
            field::CONSIGNMENT_KIND,
        )?,
        test: flag(map)?,
        barcode: optional_text(map, "barcode", field::BARCODE)?,
        reference_id: optional_text(map, "reference_id", field::REFERENCE_ID)?,
        error_code: optional_text(map, "error_code", field::ERROR_CODE)?,
        note: optional_text(map, "note", field::NOTE)?,
    };
    Ok(draft::metadata(
        header.build(),
        dispatches(map, attachment_count)?,
    ))
}

/// `TESZT`, which is required here: the writer emits it in every document.
fn flag(map: &Map<String, Value>) -> Result<bool, Failure> {
    required(map, "test", field::TEST, None)?
        .as_bool()
        .ok_or_else(|| Failure::manifest(MANIFEST_TYPE, field::TEST))
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
    let items = value
        .as_array()
        .ok_or_else(|| Failure::manifest(MANIFEST_TYPE, field::DISPATCHES))?;
    let mut dispatches = Vec::with_capacity(items.len());
    for item in items {
        let block = object(item, field::DISPATCHES, None)?;
        known_keys(block, &DISPATCH_KEYS, field::DISPATCHES, None)?;
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
    let items = value
        .as_array()
        .ok_or_else(|| Failure::manifest(MANIFEST_TYPE, field::ATTACHMENTS))?;
    let mut attachments = Vec::with_capacity(items.len());
    for (position, item) in items.iter().enumerate() {
        let index = u32::try_from(position).unwrap_or(u32::MAX);
        let map = object(item, field::ATTACHMENTS, Some(index))?;
        known_keys(map, &ATTACHMENT_KEYS, field::ATTACHMENTS, Some(index))?;
        attachments.push(Attachment {
            path: text(
                required(map, "path", field::PATH, Some(index))?,
                field::PATH,
                Some(index),
            )?,
            file_name: optional_at(map, "file_name", field::FILE_NAME, index)?,
            description: optional_at(map, "description", field::DESCRIPTION, index)?,
        });
    }
    Ok(attachments)
}

/// `YYYY-MM-DDTHH:MM:SS`, converted to the two fields a ZIP record holds.
///
/// The shape is fixed: no time zone, no fractional second, no other separator.
/// openKRX has no clock, so there is no default and no "now": a manifest that
/// omits the field is refused rather than stamped with the machine's time,
/// which would make two runs over the same manifest produce different bytes.
fn timestamp(text: String) -> Result<FixedTimestamp, Failure> {
    let refuse = || Failure::manifest(MANIFEST_TIMESTAMP, field::TIMESTAMP);
    let bytes = text.as_bytes();
    if bytes.len() != TIMESTAMP_LENGTH {
        return Err(refuse());
    }
    for (position, byte) in bytes.iter().enumerate() {
        let expected = match position {
            4 | 7 => b'-',
            10 => b'T',
            13 | 16 => b':',
            _ => {
                if byte.is_ascii_digit() {
                    continue;
                }
                return Err(refuse());
            }
        };
        if *byte != expected {
            return Err(refuse());
        }
    }
    let number = |from: usize, to: usize| text[from..to].parse::<u32>().map_err(|_| refuse());
    let small = |from: usize, to: usize| {
        number(from, to).and_then(|value| u8::try_from(value).map_err(|_| refuse()))
    };
    let year = u16::try_from(number(0, 4)?).map_err(|_| refuse())?;
    FixedTimestamp::from_parts(
        year,
        small(5, 7)?,
        small(8, 10)?,
        small(11, 13)?,
        small(14, 16)?,
        small(17, 19)?,
    )
    .map_err(|_| refuse())
}

/// Refuse a key the schema does not define, naming the object it was in.
///
/// The unknown key is deliberately not reported: it is text the manifest's
/// author wrote, and a diagnostic carries no part of the input. Naming the
/// object is enough to find a typo, and keeps the line safe to log.
fn known_keys(
    map: &Map<String, Value>,
    allowed: &[&str],
    field: &'static str,
    attachment: Option<u32>,
) -> Result<(), Failure> {
    if map.keys().all(|key| allowed.contains(&key.as_str())) {
        return Ok(());
    }
    Err(match attachment {
        Some(index) => Failure::manifest_at(MANIFEST_UNKNOWN_FIELD, field, index),
        None => Failure::manifest(MANIFEST_UNKNOWN_FIELD, field),
    })
}

/// A field that must be present and must not be null.
fn required<'a>(
    map: &'a Map<String, Value>,
    key: &str,
    field: &'static str,
    attachment: Option<u32>,
) -> Result<&'a Value, Failure> {
    match map.get(key) {
        Some(Value::Null) | None => Err(match attachment {
            Some(index) => Failure::manifest_at(MANIFEST_MISSING_FIELD, field, index),
            None => Failure::manifest(MANIFEST_MISSING_FIELD, field),
        }),
        Some(value) => Ok(value),
    }
}

/// A value that must be a JSON object.
fn object<'a>(
    value: &'a Value,
    field: &'static str,
    attachment: Option<u32>,
) -> Result<&'a Map<String, Value>, Failure> {
    value.as_object().ok_or(match attachment {
        Some(index) => Failure::manifest_at(MANIFEST_TYPE, field, index),
        None => Failure::manifest(MANIFEST_TYPE, field),
    })
}

/// A value that must be a JSON string.
fn text(value: &Value, field: &'static str, attachment: Option<u32>) -> Result<String, Failure> {
    value.as_str().map(str::to_owned).ok_or(match attachment {
        Some(index) => Failure::manifest_at(MANIFEST_TYPE, field, index),
        None => Failure::manifest(MANIFEST_TYPE, field),
    })
}

/// A required string field of `metadata`.
fn required_text(
    map: &Map<String, Value>,
    key: &str,
    field: &'static str,
) -> Result<String, Failure> {
    text(required(map, key, field, None)?, field, None)
}

/// An optional string field: absent and `null` are the same thing.
fn optional_text(
    map: &Map<String, Value>,
    key: &str,
    field: &'static str,
) -> Result<Option<String>, Failure> {
    match map.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => text(value, field, None).map(Some),
    }
}

/// The same, inside one element of the `attachments` array.
fn optional_at(
    map: &Map<String, Value>,
    key: &str,
    field: &'static str,
    attachment: u32,
) -> Result<Option<String>, Failure> {
    match map.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => text(value, field, Some(attachment)).map(Some),
    }
}

/// A token that must be one of a schema-fixed set.
///
/// The refusal names the field, and the advice sentence for
/// `manifest.invalid.enumeration` lists the tokens: what the manifest actually
/// wrote is content, and is never echoed.
fn enumeration<T>(
    parse: impl Fn(&str) -> Option<T>,
    token: &str,
    field: &'static str,
) -> Result<T, Failure> {
    parse(token).ok_or_else(|| Failure::manifest(MANIFEST_ENUMERATION, field))
}
