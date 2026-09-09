//! Bounded, schema-shaped parsing of a `KER_META_V0_9` metadata document.
//!
//! [`parse`] reads a caller-supplied byte image of one XML document and returns
//! the typed shape rules M1 to M8 of `docs/profile.md` describe. It performs no
//! filesystem, clock, process or network access, resolves nothing external, and
//! never labels the document — or the archive it came from — conforming: rules
//! M11 to M15 are unresolved, and a parsed document is an observation.
//!
//! The reader is deliberately hostile-input first. A `<!DOCTYPE ...>`
//! declaration, an entity reference other than the five XML predefines, a
//! processing instruction and a non-UTF-8 encoding are each refused with their
//! own stable code, and depth, element count, attribute count and character
//! data are counted while streaming, never taken from a declaration.
//!
//! ```
//! use openkrx_core::{MetadataLimits, metadata};
//!
//! let error = metadata::parse(b"<a/>", &MetadataLimits::DEFAULT).unwrap_err();
//! assert_eq!(error.code(), "metadata.unsupported.namespace");
//! ```

mod error;
mod field;
mod limits;
mod model;
mod reader;
mod scanner;

pub use error::{MetadataError, XmlLimitKind, XmlMalformedKind, XmlUnsupportedKind};
pub use field::MetadataField;
pub use limits::MetadataLimits;
pub use model::{AttachmentReference, ConsignmentKind, Dispatch, Header, Metadata, SourceSystem};

/// The metadata target namespace (M1).
pub const TARGET_NAMESPACE: &str = scanner::TARGET_NAMESPACE;

/// Parse one metadata document from `bytes`.
///
/// The whole document must already be in memory; the core crate never opens a
/// path. Peak additional memory is the parsed value plus one XML event at a
/// time, both bounded by [`MetadataLimits`].
///
/// # Errors
///
/// Returns a [`MetadataError`] whose [`code`](MetadataError::code)
/// distinguishes unsupported XML features, malformed documents and exceeded
/// limits. A successful result is not a statement that the document, or its
/// archive, conforms to any profile.
///
/// ```
/// use openkrx_core::{MetadataLimits, metadata};
///
/// let mut limits = MetadataLimits::DEFAULT;
/// limits.max_depth = 1;
/// let document = br#"<KULDEMENY xmlns="http://xsd.orfk.hu/rzs/ker/kuldemeny"><FEJRESZ/></KULDEMENY>"#;
/// let error = metadata::parse(document, &limits).unwrap_err();
/// assert_eq!(error.code(), "metadata.over_limit.depth");
/// ```
pub fn parse(bytes: &[u8], limits: &MetadataLimits) -> Result<Metadata, MetadataError> {
    reader::parse(bytes, limits)
}
