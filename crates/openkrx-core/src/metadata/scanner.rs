//! Bounded XML event source.
//!
//! The scanner wraps a [`quick_xml::NsReader`] and reduces its event stream to
//! the three things the grammar needs: an element opening, an element closing,
//! and character data. Everything a hostile document could use to make the
//! reader do work elsewhere is refused here, at the event boundary:
//!
//! - a `<!DOCTYPE ...>` declaration is refused outright, so no entity is ever
//!   declared and no external identifier is ever seen, let alone resolved;
//! - a general entity reference other than the five XML predefined ones and
//!   numeric character references is refused, which is what an
//!   external-entity attack must produce first;
//! - a processing instruction other than the XML declaration is refused;
//! - depth, element count, per-element attribute count and total character
//!   data are counted while streaming, against produced values only. Element
//!   text, CDATA and attribute values are all measured raw before they are
//!   materialised, so the copy taken from one event never exceeds
//!   `max_text_bytes`; a general entity reference resolves to at most one
//!   character, so it needs no such probe.
//!
//! quick-xml resolves no external identifier and opens no stream of its own,
//! and this crate performs no filesystem, clock, process or network access, so
//! a resolved entity would have nothing to reach even in principle.

use quick_xml::NsReader;
use quick_xml::XmlVersion;
use quick_xml::events::attributes::Attribute;
use quick_xml::events::{BytesDecl, BytesRef, BytesStart, Event};
use quick_xml::name::ResolveResult;

use super::error::{MetadataError, XmlLimitKind, XmlMalformedKind, XmlUnsupportedKind};
use super::limits::MetadataLimits;

/// The metadata target namespace (M1).
pub(crate) const TARGET_NAMESPACE: &str = "http://xsd.orfk.hu/rzs/ker/kuldemeny";

/// How an element's name is bound to a namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Binding {
    /// Bound to the metadata target namespace (M1).
    Target,
    /// Bound to no namespace at all, as `KEZELESI_UTASITASOK` is (M8).
    Unbound,
    /// Bound to some other namespace.
    Foreign,
}

/// One reduced event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Node {
    /// An element opened.
    Open {
        /// Namespace binding of the element name.
        binding: Binding,
        /// Local name of the element.
        name: String,
    },
    /// The innermost open element closed.
    Close,
    /// Character data, already unescaped.
    Text(String),
    /// The document ended.
    Eof,
}

/// A bounded reader over one metadata document.
pub(crate) struct Scanner<'a> {
    reader: NsReader<&'a [u8]>,
    limits: MetadataLimits,
    depth: u32,
    elements: u32,
    text_bytes: u64,
    /// Set when an empty element still owes its closing event.
    pending_close: bool,
    /// Set once the reader has produced [`Node::Eof`].
    finished: bool,
}

impl<'a> Scanner<'a> {
    /// Create a scanner over `bytes`, rejecting an oversized document first.
    pub(crate) fn new(bytes: &'a [u8], limits: &MetadataLimits) -> Result<Self, MetadataError> {
        let length = bytes.len() as u64;
        if length > limits.max_document_bytes {
            return Err(MetadataError::over_limit(
                XmlLimitKind::DocumentBytes,
                limits.max_document_bytes,
                length,
            ));
        }
        let mut reader = NsReader::from_reader(bytes);
        let config = reader.config_mut();
        config.check_end_names = true;
        config.allow_unmatched_ends = false;
        config.allow_dangling_amp = false;
        config.check_comments = true;
        Ok(Self {
            reader,
            limits: *limits,
            depth: 0,
            elements: 0,
            text_bytes: 0,
            pending_close: false,
            finished: false,
        })
    }

    /// Produce the next reduced event, enforcing every limit on the way.
    pub(crate) fn next(&mut self) -> Result<Node, MetadataError> {
        if self.pending_close {
            self.pending_close = false;
            self.depth = self.depth.saturating_sub(1);
            return Ok(Node::Close);
        }
        loop {
            if self.finished {
                return Ok(Node::Eof);
            }
            if let Some(node) = self.step()? {
                return Ok(node);
            }
        }
    }

    /// Read one raw event and reduce it, or report that it carried nothing.
    fn step(&mut self) -> Result<Option<Node>, MetadataError> {
        let mut buffer = Vec::new();
        let (resolved, event) = self
            .reader
            .read_resolved_event_into(&mut buffer)
            .map_err(map_reader_error)?;
        let binding = match event {
            Event::Start(_) | Event::Empty(_) => Some(classify(&resolved)?),
            _ => None,
        };
        match event {
            Event::Start(start) => {
                let name = self.open(&start)?;
                Ok(Some(Node::Open {
                    binding: binding.unwrap_or(Binding::Foreign),
                    name,
                }))
            }
            Event::Empty(start) => {
                let name = self.open(&start)?;
                self.pending_close = true;
                Ok(Some(Node::Open {
                    binding: binding.unwrap_or(Binding::Foreign),
                    name,
                }))
            }
            Event::End(_) => {
                self.depth = self.depth.saturating_sub(1);
                Ok(Some(Node::Close))
            }
            Event::Text(text) => {
                self.reserve_text(text.len() as u64)?;
                self.text(&text.xml10_content())
            }
            Event::CData(data) => {
                self.reserve_text(data.len() as u64)?;
                self.text(&data.xml10_content())
            }
            Event::GeneralRef(reference) => {
                let resolved_text = resolve_reference(&reference)?;
                self.text(&resolved_text)
            }
            Event::DocType(_) => Err(MetadataError::unsupported(XmlUnsupportedKind::DocType)),
            Event::PI(_) => Err(MetadataError::unsupported(
                XmlUnsupportedKind::ProcessingInstruction,
            )),
            Event::Decl(declaration) => check_declaration(&declaration).map(|()| None),
            Event::Comment(_) => Ok(None),
            Event::Eof => {
                self.finished = true;
                Ok(Some(Node::Eof))
            }
        }
    }

    /// Charge character data against the document ceiling and emit it.
    fn text(&mut self, content: &str) -> Result<Option<Node>, MetadataError> {
        if content.is_empty() {
            return Ok(None);
        }
        self.charge_text(content.len() as u64)?;
        Ok(Some(Node::Text(content.to_owned())))
    }

    /// Account for an opened element and return its local name.
    fn open(&mut self, start: &BytesStart<'_>) -> Result<String, MetadataError> {
        self.elements = self.elements.saturating_add(1);
        if self.elements > self.limits.max_elements {
            return Err(MetadataError::over_limit(
                XmlLimitKind::Elements,
                u64::from(self.limits.max_elements),
                u64::from(self.elements),
            ));
        }
        self.depth = self.depth.saturating_add(1);
        if self.depth > self.limits.max_depth {
            return Err(MetadataError::over_limit(
                XmlLimitKind::Depth,
                u64::from(self.limits.max_depth),
                u64::from(self.depth),
            ));
        }
        self.check_attributes(start)?;
        Ok(start.local_name().into_inner().to_owned())
    }

    /// Count attributes and reject malformed or over-limit attribute lists.
    ///
    /// Every value is unescaped here rather than lazily, so an entity hidden in
    /// an attribute cannot slip past the entity rule above. The value's raw
    /// length is reserved before it is normalised, and its normalised length is
    /// charged afterwards, so attribute text is bounded exactly as element
    /// text is; the list is counted first, so the count limit is reported
    /// ahead of the text limit when an element crosses both.
    fn check_attributes(&mut self, start: &BytesStart<'_>) -> Result<(), MetadataError> {
        // The whole list is counted before any of its text is charged, so an
        // element carrying too many attributes reports the count rather than
        // the text bound it may cross on the way there.
        let mut count = 0_u32;
        for attribute in start.attributes().with_checks(true) {
            attribute.map_err(|_| MetadataError::malformed(XmlMalformedKind::Attribute))?;
            count = count.saturating_add(1);
            if count > self.limits.max_attributes_per_element {
                return Err(MetadataError::over_limit(
                    XmlLimitKind::AttributesPerElement,
                    u64::from(self.limits.max_attributes_per_element),
                    u64::from(count),
                ));
            }
        }
        for attribute in start.attributes().with_checks(true) {
            let attribute: Attribute<'_> =
                attribute.map_err(|_| MetadataError::malformed(XmlMalformedKind::Attribute))?;
            self.reserve_text(attribute.value.len() as u64)?;
            let value = attribute
                .normalized_value(XmlVersion::Implicit1_0)
                .map_err(|_| MetadataError::malformed(XmlMalformedKind::Entity))?;
            self.charge_text(value.len() as u64)?;
        }
        Ok(())
    }

    /// Refuse an event whose raw bytes cannot fit the remaining headroom.
    ///
    /// This runs before an event is unescaped or copied, so the copy
    /// materialised from one event is bounded by `max_text_bytes`. The raw
    /// event itself is already in the reader's buffer by then and stays
    /// bounded by `max_document_bytes`. Unescaping and end-of-line
    /// normalisation never grow a value, so a raw length within the headroom
    /// guarantees the materialised one is too; the converse does not hold, and
    /// an event whose raw form exceeds the headroom is refused even when its
    /// materialised form would have fitted.
    fn reserve_text(&self, raw_bytes: u64) -> Result<(), MetadataError> {
        let projected = self.text_bytes.saturating_add(raw_bytes);
        if projected > self.limits.max_text_bytes {
            return Err(MetadataError::over_limit(
                XmlLimitKind::TextBytes,
                self.limits.max_text_bytes,
                projected,
            ));
        }
        Ok(())
    }

    /// Charge `bytes` against the document-wide character-data ceiling.
    fn charge_text(&mut self, bytes: u64) -> Result<(), MetadataError> {
        self.text_bytes = self.text_bytes.checked_add(bytes).ok_or_else(|| {
            MetadataError::over_limit(
                XmlLimitKind::TextBytes,
                self.limits.max_text_bytes,
                u64::MAX,
            )
        })?;
        if self.text_bytes > self.limits.max_text_bytes {
            return Err(MetadataError::over_limit(
                XmlLimitKind::TextBytes,
                self.limits.max_text_bytes,
                self.text_bytes,
            ));
        }
        Ok(())
    }
}

/// Classify a resolved namespace, refusing a prefix that was never bound.
fn classify(resolved: &ResolveResult<'_>) -> Result<Binding, MetadataError> {
    match resolved {
        ResolveResult::Bound(namespace) if namespace.as_ref() == TARGET_NAMESPACE => {
            Ok(Binding::Target)
        }
        ResolveResult::Bound(_) => Ok(Binding::Foreign),
        ResolveResult::Unbound => Ok(Binding::Unbound),
        ResolveResult::Unknown(_) => Err(MetadataError::malformed(XmlMalformedKind::UnboundPrefix)),
    }
}

/// Resolve a general reference, accepting only predefined and numeric forms.
///
/// Anything else would need a document type definition to have declared it,
/// which this reader refuses outright, so an undeclared name is malformed.
fn resolve_reference(reference: &BytesRef<'_>) -> Result<String, MetadataError> {
    if reference.is_char_ref() {
        return match reference.resolve_char_ref() {
            Ok(Some(character)) => Ok(character.to_string()),
            Ok(None) | Err(_) => Err(MetadataError::malformed(XmlMalformedKind::Entity)),
        };
    }
    match &**reference {
        "amp" => Ok("&".to_owned()),
        "lt" => Ok("<".to_owned()),
        "gt" => Ok(">".to_owned()),
        "apos" => Ok("'".to_owned()),
        "quot" => Ok("\"".to_owned()),
        _ => Err(MetadataError::malformed(XmlMalformedKind::Entity)),
    }
}

/// Accept only a UTF-8 XML declaration; this reader decodes nothing else.
fn check_declaration(declaration: &BytesDecl<'_>) -> Result<(), MetadataError> {
    let Some(encoding) = declaration.encoding() else {
        return Ok(());
    };
    let encoding = encoding.map_err(|_| MetadataError::malformed(XmlMalformedKind::Attribute))?;
    if encoding.eq_ignore_ascii_case("utf-8") || encoding.eq_ignore_ascii_case("utf8") {
        return Ok(());
    }
    Err(MetadataError::unsupported(XmlUnsupportedKind::Encoding))
}

/// Map a quick-xml error onto a stable, content-free code.
fn map_reader_error(error: quick_xml::Error) -> MetadataError {
    match error {
        quick_xml::Error::Escape(_) => MetadataError::malformed(XmlMalformedKind::Entity),
        quick_xml::Error::Encoding(_) => MetadataError::malformed(XmlMalformedKind::Encoding),
        quick_xml::Error::InvalidAttr(_) => MetadataError::malformed(XmlMalformedKind::Attribute),
        quick_xml::Error::Namespace(_) => MetadataError::malformed(XmlMalformedKind::UnboundPrefix),
        quick_xml::Error::Io(_) | quick_xml::Error::Syntax(_) | quick_xml::Error::IllFormed(_) => {
            MetadataError::malformed(XmlMalformedKind::Syntax)
        }
    }
}
