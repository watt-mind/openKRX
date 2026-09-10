//! Strict reading of the two JSON documents openKRX takes.
//!
//! `create` reads a manifest and `repack` reads a list of edits. Both are
//! schema-fixed documents a person wrote, and both are validated the same way,
//! in both directions: a required field that is absent is refused, and a key
//! the schema does not define is refused rather than ignored. A manifest whose
//! `atachments` key was silently dropped would produce a package with no
//! attachment and a success report, which is the worst outcome available.
//!
//! Every refusal names the *field*, from the fixed set of schema paths the
//! calling module defines — never the value, and never the unknown key itself,
//! so a diagnostic stays as safe to log as any other. Nothing here reads a
//! file, resolves a path or consults a clock.

use openkrx_core::create::FixedTimestamp;
use serde_json::{Map, Value};

use crate::exit::{
    Failure, MANIFEST_ENUMERATION, MANIFEST_MISSING_FIELD, MANIFEST_SCHEMA_VERSION,
    MANIFEST_SYNTAX, MANIFEST_TIMESTAMP, MANIFEST_TYPE, MANIFEST_UNKNOWN_FIELD,
};

/// The only schema version either document may declare.
pub const SCHEMA_VERSION: u64 = 1;

/// The characters of `YYYY-MM-DDTHH:MM:SS`.
const TIMESTAMP_LENGTH: usize = 19;

/// The document object, or `manifest.invalid.syntax`.
pub fn document(bytes: &[u8], root: &'static str) -> Result<Map<String, Value>, Failure> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|_| Failure::manifest(MANIFEST_SYNTAX, root))?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| Failure::manifest(MANIFEST_SYNTAX, root))
}

/// `schema_version` must be exactly the version this build implements.
pub fn schema_version(map: &Map<String, Value>, field: &'static str) -> Result<(), Failure> {
    let declared = required(map, "schema_version", field, None)?
        .as_u64()
        .ok_or_else(|| Failure::manifest(MANIFEST_TYPE, field))?;
    if declared != SCHEMA_VERSION {
        return Err(Failure::manifest(MANIFEST_SCHEMA_VERSION, field));
    }
    Ok(())
}

/// Refuse a key the schema does not define, naming the object it was in.
///
/// The unknown key is deliberately not reported: it is text the document's
/// author wrote, and a diagnostic carries no part of the input. Naming the
/// object is enough to find a typo, and keeps the line safe to log.
pub fn known_keys(
    map: &Map<String, Value>,
    allowed: &[&str],
    field: &'static str,
    attachment: Option<u32>,
) -> Result<(), Failure> {
    if map.keys().all(|key| allowed.contains(&key.as_str())) {
        return Ok(());
    }
    Err(scoped(MANIFEST_UNKNOWN_FIELD, field, attachment))
}

/// A field that must be present and must not be null.
pub fn required<'a>(
    map: &'a Map<String, Value>,
    key: &str,
    field: &'static str,
    attachment: Option<u32>,
) -> Result<&'a Value, Failure> {
    match map.get(key) {
        Some(Value::Null) | None => Err(scoped(MANIFEST_MISSING_FIELD, field, attachment)),
        Some(value) => Ok(value),
    }
}

/// A value that must be a JSON object.
pub fn object<'a>(
    value: &'a Value,
    field: &'static str,
    attachment: Option<u32>,
) -> Result<&'a Map<String, Value>, Failure> {
    value
        .as_object()
        .ok_or_else(|| scoped(MANIFEST_TYPE, field, attachment))
}

/// A value that must be a JSON array.
pub fn array<'a>(value: &'a Value, field: &'static str) -> Result<&'a Vec<Value>, Failure> {
    value
        .as_array()
        .ok_or_else(|| Failure::manifest(MANIFEST_TYPE, field))
}

/// A value that must be a JSON string.
pub fn text(
    value: &Value,
    field: &'static str,
    attachment: Option<u32>,
) -> Result<String, Failure> {
    value
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| scoped(MANIFEST_TYPE, field, attachment))
}

/// A required string field.
pub fn required_text(
    map: &Map<String, Value>,
    key: &str,
    field: &'static str,
) -> Result<String, Failure> {
    text(required(map, key, field, None)?, field, None)
}

/// An optional string field: absent and `null` are the same thing.
pub fn optional_text(
    map: &Map<String, Value>,
    key: &str,
    field: &'static str,
) -> Result<Option<String>, Failure> {
    match map.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => text(value, field, None).map(Some),
    }
}

/// The same, inside one element of an array.
pub fn optional_at(
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

/// A required boolean field.
pub fn flag(map: &Map<String, Value>, key: &str, field: &'static str) -> Result<bool, Failure> {
    required(map, key, field, None)?
        .as_bool()
        .ok_or_else(|| Failure::manifest(MANIFEST_TYPE, field))
}

/// A token that must be one of a schema-fixed set.
///
/// The refusal names the field, and the advice sentence for
/// `manifest.invalid.enumeration` lists the tokens: what the document actually
/// wrote is content, and is never echoed.
pub fn enumeration<T>(
    parse: impl Fn(&str) -> Option<T>,
    token: &str,
    field: &'static str,
) -> Result<T, Failure> {
    parse(token).ok_or_else(|| Failure::manifest(MANIFEST_ENUMERATION, field))
}

/// `YYYY-MM-DDTHH:MM:SS`, converted to the two fields a ZIP record holds.
///
/// The shape is fixed: no time zone, no fractional second, no other separator.
/// openKRX has no clock, so there is no default and no "now": a document that
/// omits the field is refused rather than stamped with the machine's time,
/// which would make two runs over the same input produce different bytes.
pub fn timestamp(text: &str, field: &'static str) -> Result<FixedTimestamp, Failure> {
    let refuse = || Failure::manifest(MANIFEST_TIMESTAMP, field);
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

/// A failure scoped to one element of an array, when there is one.
fn scoped(code: &'static str, field: &'static str, attachment: Option<u32>) -> Failure {
    match attachment {
        Some(index) => Failure::manifest_at(code, field, index),
        None => Failure::manifest(code, field),
    }
}
