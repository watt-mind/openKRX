//! Stable, content-free metadata error reporting.
//!
//! Every error exposes a dotted [`MetadataError::code`] grouped by category:
//! `metadata.unsupported.*`, `metadata.malformed.*` and
//! `metadata.over_limit.*`. Unsupported XML features, malformed documents and
//! resource exhaustion stay distinguishable.
//!
//! [`fmt::Display`] prints the code and numeric limit values only. Element
//! text, attribute values, attribute names, entity names and field values are
//! never printed, because a metadata document describes real correspondence.
//! The schema-fixed [`MetadataField`] an error concerns is available as a typed
//! field for consumers that need it.

use core::fmt;

use super::field::MetadataField;

/// An XML feature this reader deliberately refuses to process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum XmlUnsupportedKind {
    /// A `<!DOCTYPE ...>` declaration. Never processed, never resolved.
    DocType,
    /// A processing instruction other than the XML declaration.
    ProcessingInstruction,
    /// A character encoding other than UTF-8.
    Encoding,
    /// The root element is bound to a namespace the profile does not define.
    Namespace,
}

impl XmlUnsupportedKind {
    /// Return the stable dotted identifier for this variant.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::DocType => "metadata.unsupported.dtd",
            Self::ProcessingInstruction => "metadata.unsupported.processing_instruction",
            Self::Encoding => "metadata.unsupported.encoding",
            Self::Namespace => "metadata.unsupported.namespace",
        }
    }
}

/// A document that does not match the grammar, or is not well-formed XML.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum XmlMalformedKind {
    /// The byte stream is not well-formed XML.
    Syntax,
    /// The byte stream is not valid UTF-8.
    Encoding,
    /// A general entity reference other than the five predefined ones.
    Entity,
    /// A namespace prefix was used without being bound.
    UnboundPrefix,
    /// An attribute is malformed or repeated on one element.
    Attribute,
    /// The root element is not `KULDEMENY` (M1).
    RootElement,
    /// A required element is absent (M3, M5, M7).
    MissingElement,
    /// An element appears more than once where the grammar allows one.
    DuplicateElement,
    /// Children appear in an order the grammar does not allow (M2).
    ElementOrder,
    /// A leaf element that must carry text carries child elements instead.
    UnexpectedChild,
    /// A value is outside its schema enumeration (M4).
    Enumeration,
    /// A value declared `xs:long` is not an integer (M5, M7).
    Integer,
    /// A value declared `xs:boolean` is not a boolean (M3).
    Boolean,
}

impl XmlMalformedKind {
    /// Return the stable dotted identifier for this variant.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Syntax => "metadata.malformed.syntax",
            Self::Encoding => "metadata.malformed.encoding",
            Self::Entity => "metadata.malformed.entity",
            Self::UnboundPrefix => "metadata.malformed.unbound_prefix",
            Self::Attribute => "metadata.malformed.attribute",
            Self::RootElement => "metadata.malformed.root_element",
            Self::MissingElement => "metadata.malformed.missing_element",
            Self::DuplicateElement => "metadata.malformed.duplicate_element",
            Self::ElementOrder => "metadata.malformed.element_order",
            Self::UnexpectedChild => "metadata.malformed.unexpected_child",
            Self::Enumeration => "metadata.malformed.enumeration",
            Self::Integer => "metadata.malformed.integer",
            Self::Boolean => "metadata.malformed.boolean",
        }
    }
}

/// Which documented metadata limit a document exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum XmlLimitKind {
    /// `max_document_bytes`.
    DocumentBytes,
    /// `max_depth`.
    Depth,
    /// `max_elements`.
    Elements,
    /// `max_attributes_per_element`.
    AttributesPerElement,
    /// `max_text_bytes`.
    TextBytes,
}

impl XmlLimitKind {
    /// Return the stable dotted identifier for this variant.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::DocumentBytes => "metadata.over_limit.document_bytes",
            Self::Depth => "metadata.over_limit.depth",
            Self::Elements => "metadata.over_limit.elements",
            Self::AttributesPerElement => "metadata.over_limit.attributes_per_element",
            Self::TextBytes => "metadata.over_limit.text_bytes",
        }
    }
}

/// Why a metadata document could not be parsed.
///
/// The enum is `#[non_exhaustive]`: match on [`MetadataError::code`] for stable
/// behaviour across versions, and treat an unknown code as a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MetadataError {
    /// A documented limit was reached.
    OverLimit {
        /// Which limit.
        limit: XmlLimitKind,
        /// The configured ceiling.
        limit_value: u64,
        /// The value that reached or exceeded it, when meaningful.
        observed: Option<u64>,
    },
    /// The document uses a feature this reader deliberately does not process.
    Unsupported {
        /// Which feature.
        kind: XmlUnsupportedKind,
    },
    /// The document is not well-formed, or does not match the grammar.
    Malformed {
        /// Which contradiction.
        kind: XmlMalformedKind,
        /// The schema-fixed element the problem concerns, when one applies.
        field: Option<MetadataField>,
    },
}

impl MetadataError {
    /// Return the stable dotted identifier for this error.
    ///
    /// ```
    /// use openkrx_core::{MetadataLimits, metadata};
    ///
    /// let error = metadata::parse(b"<!DOCTYPE a><a/>", &MetadataLimits::DEFAULT).unwrap_err();
    /// assert_eq!(error.code(), "metadata.unsupported.dtd");
    /// ```
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match *self {
            Self::OverLimit { limit, .. } => limit.code(),
            Self::Unsupported { kind } => kind.code(),
            Self::Malformed { kind, .. } => kind.code(),
        }
    }

    /// The schema-fixed element the error concerns, when one applies.
    #[must_use]
    pub const fn field(&self) -> Option<MetadataField> {
        match *self {
            Self::Malformed { field, .. } => field,
            Self::OverLimit { .. } | Self::Unsupported { .. } => None,
        }
    }

    /// Construct a malformed error for `kind` without a field.
    pub(crate) const fn malformed(kind: XmlMalformedKind) -> Self {
        Self::Malformed { kind, field: None }
    }

    /// Construct a malformed error for `kind` concerning `field`.
    pub(crate) const fn about(kind: XmlMalformedKind, field: MetadataField) -> Self {
        Self::Malformed {
            kind,
            field: Some(field),
        }
    }

    /// Construct an unsupported error for `kind`.
    pub(crate) const fn unsupported(kind: XmlUnsupportedKind) -> Self {
        Self::Unsupported { kind }
    }

    /// Construct a limit error for `limit`.
    pub(crate) const fn over_limit(limit: XmlLimitKind, limit_value: u64, observed: u64) -> Self {
        Self::OverLimit {
            limit,
            limit_value,
            observed: Some(observed),
        }
    }
}

impl fmt::Display for MetadataError {
    /// Print the code and numeric limit values only.
    ///
    /// Document content of every kind is deliberately excluded, so a diagnostic
    /// can be logged without leaking package content. Even the schema-fixed
    /// field is withheld here; read [`MetadataError::field`] instead.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())?;
        if let Self::OverLimit {
            limit_value,
            observed,
            ..
        } = *self
        {
            write!(f, " (limit {limit_value}")?;
            if let Some(observed) = observed {
                write!(f, ", observed {observed}")?;
            }
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl std::error::Error for MetadataError {}
