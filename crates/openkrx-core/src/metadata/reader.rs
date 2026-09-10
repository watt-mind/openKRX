//! The `KER_META_V0_9` grammar, re-expressed as code.
//!
//! Each function below carries the `docs/profile.md` rule it encodes. No XSD is
//! vendored: the rules are cited, not copied.
//!
//! Two deliberate leniencies, both required by unresolved rules:
//!
//! - Sibling order is enforced only for the `KULDEMENY` children, because M2 is
//!   the one rule that states an order. Inside `FEJRESZ` and `MELLEKLET` the
//!   reader accepts any order and rejects only repetition.
//! - `TESZT` and `MELLEKLET_LEIRASA` may be absent, and `MERET` need not be a
//!   number. One official example omits or mis-types exactly these (M11), so
//!   their absence is recorded for `crate::profile` to report as unresolved
//!   rather than treated as invalid.
//!
//! Leaf text is concatenated and trimmed of ASCII whitespace, so a value split
//! by a character reference or a CDATA section reads the same as a plain one.

use core::mem;

use super::error::{MetadataError, XmlMalformedKind, XmlUnsupportedKind};
use super::field::MetadataField;
use super::limits::MetadataLimits;
use super::model::{
    AttachmentReference, ConsignmentKind, Dispatch, Header, Metadata, SourceSystem,
};
use super::scanner::{Binding, Node, Scanner};

/// Position of each `KULDEMENY` child in the sequence M2 fixes.
const KULDEMENY_SEQUENCE: [MetadataField; 5] = [
    MetadataField::Fejresz,
    MetadataField::Erkeztetes,
    MetadataField::Bontasok,
    MetadataField::Expedialasok,
    MetadataField::Tertiveveny,
];

/// Parse a document, or say precisely why it is not one.
pub(crate) fn parse(bytes: &[u8], limits: &MetadataLimits) -> Result<Metadata, MetadataError> {
    let mut parser = Parser {
        scanner: Scanner::new(bytes, limits)?,
        unknown_elements: 0,
    };
    parser.document()
}

/// Grammar state over a bounded event source.
struct Parser<'a> {
    scanner: Scanner<'a>,
    unknown_elements: u32,
}

impl Parser<'_> {
    /// Read the whole document: prolog, root element, trailing miscellany.
    fn document(&mut self) -> Result<Metadata, MetadataError> {
        let root = loop {
            match self.scanner.next()? {
                Node::Open { binding, name } => break (binding, name),
                Node::Text(_) => {}
                Node::Close | Node::Eof => {
                    return Err(MetadataError::about(
                        XmlMalformedKind::MissingElement,
                        MetadataField::Kuldemeny,
                    ));
                }
            }
        };
        self.root(root.0, &root.1)
    }

    /// Check the root against M1 and read its children.
    fn root(&mut self, binding: Binding, name: &str) -> Result<Metadata, MetadataError> {
        if binding != Binding::Target {
            return Err(MetadataError::unsupported(XmlUnsupportedKind::Namespace));
        }
        if name != MetadataField::Kuldemeny.local_name() {
            return Err(MetadataError::about(
                XmlMalformedKind::RootElement,
                MetadataField::Kuldemeny,
            ));
        }
        self.consignment()
    }

    /// Read `KULDEMENY`: `FEJRESZ` first, then the optional blocks, in order.
    fn consignment(&mut self) -> Result<Metadata, MetadataError> {
        let mut header = None;
        let mut receipt_present = false;
        let mut openings_present = false;
        let mut return_receipt_present = false;
        let mut dispatches = Vec::new();
        let mut order = Order::default();
        while let Some(child) = self.child()? {
            let Some(position) = sequence_position(&child) else {
                self.skip_unknown()?;
                continue;
            };
            let field = KULDEMENY_SEQUENCE[position];
            order.accept(position, field)?;
            match field {
                MetadataField::Fejresz => header = Some(self.header()?),
                MetadataField::Expedialasok => dispatches = self.dispatches()?,
                other => {
                    receipt_present |= other == MetadataField::Erkeztetes;
                    openings_present |= other == MetadataField::Bontasok;
                    return_receipt_present |= other == MetadataField::Tertiveveny;
                    self.skip_known()?;
                }
            }
        }
        Ok(Metadata {
            header: header.ok_or(MetadataError::about(
                XmlMalformedKind::MissingElement,
                MetadataField::Fejresz,
            ))?,
            receipt_present,
            openings_present,
            return_receipt_present,
            dispatches,
            unknown_elements: self.unknown_elements,
        })
    }

    /// Read `FEJRESZ` (M3, M4).
    fn header(&mut self) -> Result<Header, MetadataError> {
        let mut slots = Slots::default();
        while let Some(child) = self.child()? {
            let Some(field) = known_field(&child) else {
                self.skip_unknown()?;
                continue;
            };
            let text = self.leaf(field)?;
            slots.put(field, text)?;
        }
        slots.into_header()
    }

    /// Read `EXPEDIALASOK`: a list of `EXPEDIALAS` blocks (M2, M7).
    fn dispatches(&mut self) -> Result<Vec<Dispatch>, MetadataError> {
        let mut dispatches = Vec::new();
        while let Some(child) = self.child()? {
            if known_field(&child) == Some(MetadataField::Expedialas) {
                dispatches.push(self.dispatch()?);
            } else {
                self.skip_unknown()?;
            }
        }
        Ok(dispatches)
    }

    /// Read one `EXPEDIALAS` (M7, M8).
    fn dispatch(&mut self) -> Result<Dispatch, MetadataError> {
        let mut declared_attachment_count = None;
        let mut attachments = Vec::new();
        let mut attachments_present = false;
        let mut handling_instructions_unqualified = false;
        while let Some(child) = self.child()? {
            match known_field(&child) {
                Some(MetadataField::MellekletekSzama) => {
                    let text = self.leaf(MetadataField::MellekletekSzama)?;
                    if declared_attachment_count.is_some() {
                        return Err(MetadataError::about(
                            XmlMalformedKind::DuplicateElement,
                            MetadataField::MellekletekSzama,
                        ));
                    }
                    declared_attachment_count =
                        Some(integer(&text, MetadataField::MellekletekSzama)?);
                }
                Some(MetadataField::Mellekletek) => {
                    // The presence of the container is retained on the value,
                    // not just used here: an empty `MELLEKLETEK` and no
                    // `MELLEKLETEK` at all are different documents, and a
                    // writer that could not tell them apart would turn the
                    // second into the first.
                    //
                    // M7: one list per dispatch. A second one is a repeated
                    // element like any other, not a count to merge into the
                    // first: merging would report the document as a count
                    // mismatch and hide the real defect.
                    if mem::replace(&mut attachments_present, true) {
                        return Err(MetadataError::about(
                            XmlMalformedKind::DuplicateElement,
                            MetadataField::Mellekletek,
                        ));
                    }
                    attachments = self.attachments()?;
                }
                _ => {
                    // M8: the schema declares this one element unqualified, so
                    // only the unbound spelling counts as the real thing.
                    if child.binding == Binding::Unbound
                        && child.name == MetadataField::KezelesiUtasitasok.local_name()
                    {
                        handling_instructions_unqualified = true;
                        self.skip_known()?;
                    } else {
                        self.skip_unknown()?;
                    }
                }
            }
        }
        Ok(Dispatch {
            declared_attachment_count,
            attachments,
            attachments_present,
            handling_instructions_unqualified,
        })
    }

    /// Read `MELLEKLETEK`: a list of `MELLEKLET` references (M5).
    fn attachments(&mut self) -> Result<Vec<AttachmentReference>, MetadataError> {
        let mut attachments = Vec::new();
        while let Some(child) = self.child()? {
            if known_field(&child) == Some(MetadataField::Melleklet) {
                attachments.push(self.attachment()?);
            } else {
                self.skip_unknown()?;
            }
        }
        Ok(attachments)
    }

    /// Read one `MELLEKLET` (M5, M6).
    fn attachment(&mut self) -> Result<AttachmentReference, MetadataError> {
        let mut slots = Slots::default();
        while let Some(child) = self.child()? {
            let Some(field) = known_field(&child) else {
                self.skip_unknown()?;
                continue;
            };
            let text = self.leaf(field)?;
            slots.put(field, text)?;
        }
        slots.into_attachment()
    }

    /// Return the next child element of the open element, or `None` at its end.
    ///
    /// Character data between child elements is discarded: the grammar defines
    /// no mixed content, and a container may carry information this reader does
    /// not interpret (A9).
    fn child(&mut self) -> Result<Option<Child>, MetadataError> {
        loop {
            match self.scanner.next()? {
                Node::Open { binding, name } => return Ok(Some(Child { binding, name })),
                Node::Text(_) => {}
                Node::Close => return Ok(None),
                Node::Eof => return Err(MetadataError::malformed(XmlMalformedKind::Syntax)),
            }
        }
    }

    /// Consume an element the grammar does not define, counting it (A9).
    fn skip_unknown(&mut self) -> Result<(), MetadataError> {
        self.unknown_elements = self.unknown_elements.saturating_add(1);
        self.skip_known()
    }

    /// Consume the open element, and everything under it, unread.
    fn skip_known(&mut self) -> Result<(), MetadataError> {
        let mut depth = 1_u32;
        loop {
            match self.scanner.next()? {
                Node::Open { .. } => depth = depth.saturating_add(1),
                Node::Close => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(());
                    }
                }
                Node::Text(_) => {}
                Node::Eof => return Err(MetadataError::malformed(XmlMalformedKind::Syntax)),
            }
        }
    }

    /// Read the text of a leaf element, rejecting a child element inside it.
    fn leaf(&mut self, field: MetadataField) -> Result<String, MetadataError> {
        let mut text = String::new();
        loop {
            match self.scanner.next()? {
                Node::Text(part) => text.push_str(&part),
                Node::Close => return Ok(text.trim().to_owned()),
                Node::Open { .. } => {
                    return Err(MetadataError::about(
                        XmlMalformedKind::UnexpectedChild,
                        field,
                    ));
                }
                Node::Eof => return Err(MetadataError::malformed(XmlMalformedKind::Syntax)),
            }
        }
    }
}

/// An element that has just opened.
struct Child {
    binding: Binding,
    name: String,
}

/// The grammar field a child denotes, when it is in the target namespace.
fn known_field(child: &Child) -> Option<MetadataField> {
    if child.binding != Binding::Target {
        return None;
    }
    MetadataField::from_local_name(&child.name)
}

/// Position of a `KULDEMENY` child inside the M2 sequence.
fn sequence_position(child: &Child) -> Option<usize> {
    let field = known_field(child)?;
    KULDEMENY_SEQUENCE
        .iter()
        .position(|candidate| *candidate == field)
}

/// Enforcement of the M2 sequence: each block at most once, never backwards.
#[derive(Default)]
struct Order {
    seen: [bool; KULDEMENY_SEQUENCE.len()],
    reached: usize,
}

impl Order {
    /// Accept the block at `position`, or say how it breaks the sequence.
    fn accept(&mut self, position: usize, field: MetadataField) -> Result<(), MetadataError> {
        if self.seen[position] {
            return Err(MetadataError::about(
                XmlMalformedKind::DuplicateElement,
                field,
            ));
        }
        if position < self.reached {
            return Err(MetadataError::about(XmlMalformedKind::ElementOrder, field));
        }
        self.seen[position] = true;
        self.reached = position;
        Ok(())
    }
}

/// Read an `xs:long` value (M5, M7).
fn integer(text: &str, field: MetadataField) -> Result<i64, MetadataError> {
    text.parse::<i64>()
        .map_err(|_| MetadataError::about(XmlMalformedKind::Integer, field))
}

/// Read an `xs:boolean` value in either lexical form (M3).
fn boolean(text: &str) -> Result<bool, MetadataError> {
    match text {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(MetadataError::about(
            XmlMalformedKind::Boolean,
            MetadataField::Teszt,
        )),
    }
}

/// Text collected for each leaf field of one `FEJRESZ` or `MELLEKLET`.
#[derive(Default)]
struct Slots {
    entries: Vec<(MetadataField, String)>,
}

impl Slots {
    /// Record `text` for `field`, rejecting a repeated element.
    fn put(&mut self, field: MetadataField, text: String) -> Result<(), MetadataError> {
        if self.entries.iter().any(|(seen, _)| *seen == field) {
            return Err(MetadataError::about(
                XmlMalformedKind::DuplicateElement,
                field,
            ));
        }
        self.entries.push((field, text));
        Ok(())
    }

    /// Take the value recorded for `field`, if the element was present.
    fn take(&mut self, field: MetadataField) -> Option<String> {
        let position = self.entries.iter().position(|(seen, _)| *seen == field)?;
        Some(self.entries.remove(position).1)
    }

    /// Take a value the grammar requires.
    fn require(&mut self, field: MetadataField) -> Result<String, MetadataError> {
        self.take(field).ok_or(MetadataError::about(
            XmlMalformedKind::MissingElement,
            field,
        ))
    }

    /// Assemble a `FEJRESZ` (M3, M4).
    fn into_header(mut self) -> Result<Header, MetadataError> {
        let version = self.require(MetadataField::KrxVerzioszam)?;
        let source_text = self.require(MetadataField::ForrasrendszerAzonosito)?;
        let source_system = SourceSystem::parse(&source_text).ok_or(MetadataError::about(
            XmlMalformedKind::Enumeration,
            MetadataField::ForrasrendszerAzonosito,
        ))?;
        let consignment_id = self.require(MetadataField::KuldemenyAzonosito)?;
        let created_at_text = self.require(MetadataField::KuldemenyLetrehozasanakIdeje)?;
        let kind_text = self.require(MetadataField::KuldemenyTipus)?;
        let consignment_kind = ConsignmentKind::parse(&kind_text).ok_or(MetadataError::about(
            XmlMalformedKind::Enumeration,
            MetadataField::KuldemenyTipus,
        ))?;
        let test_text = self.take(MetadataField::Teszt);
        Ok(Header {
            version,
            source_system,
            consignment_id,
            created_at_text,
            consignment_kind,
            test: match &test_text {
                Some(text) => boolean(text)?,
                None => false,
            },
            test_present: test_text.is_some(),
            barcode: self.take(MetadataField::Vonalkod),
            reference_id: self.take(MetadataField::KuldemenyHivatkozasiAzonosito),
            error_code: self.take(MetadataField::Hibakod),
            note: self.take(MetadataField::KuldemenyMegjegyzes),
        })
    }

    /// Assemble a `MELLEKLET` (M5, M6).
    fn into_attachment(mut self) -> Result<AttachmentReference, MetadataError> {
        let number_text = self.require(MetadataField::CsatolmanySzama)?;
        let number = integer(&number_text, MetadataField::CsatolmanySzama)?;
        let file_name = self.require(MetadataField::FajlNev)?;
        let size_text = self.require(MetadataField::Meret)?;
        let location = self.require(MetadataField::Elhelyezkedes)?;
        Ok(AttachmentReference {
            description: self.take(MetadataField::MellekletLeirasa),
            number,
            file_name,
            size_value: size_text
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite()),
            size_text,
            location,
            quantity: self.take(MetadataField::Mennyiseg),
            quantity_unit: self.take(MetadataField::MennyisegiEgyseg),
        })
    }
}
