//! Serialising a [`Metadata`] value as a `KER_META_V0_9`-shaped document.
//!
//! The output is the shape rules M1 to M9 of `docs/profile.md` describe, and
//! nothing more:
//!
//! - the target namespace (M1) is bound to the `ns2` prefix, which is what both
//!   official examples use (M9), and every element carries it except
//!   `KEZELESI_UTASITASOK`, which the schema declares unqualified (M8);
//! - `KULDEMENY` children follow the M2 sequence, `FEJRESZ` children the M3
//!   order and `MELLEKLET` children the M5 order;
//! - the declaration is `<?xml version="1.0" encoding="UTF-8"
//!   standalone="yes"?>`, as the examples write it, and there is no DTD, no
//!   processing instruction, no comment and no insignificant whitespace, so a
//!   document is a function of its values alone.
//!
//! Two properties are load-bearing, and both are tested rather than asserted:
//! the document [`crate::metadata::parse`] reads back equals the value written,
//! and the same value always produces the same bytes.
//!
//! Text is escaped deterministically — `&`, `<` and `>`, always, in that
//! order of checking — and a value XML 1.0 cannot carry is refused with
//! `create.invalid.text` rather than dropped, replaced or escaped into
//! something the reader would return differently.

use crate::metadata::{AttachmentReference, Metadata, MetadataField, TARGET_NAMESPACE};

use super::error::{CreateError, InvalidKind};

/// The namespace prefix both official examples use (M9).
const PREFIX: &str = "ns2";
/// The XML declaration, exactly as the examples write it.
const DECLARATION: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>";

/// Serialise `metadata` as one UTF-8 document.
///
/// # Errors
///
/// `create.invalid.text` for a value holding a character XML 1.0 cannot carry,
/// and `create.invalid.untrimmed_text` for a value the reader's trimming would
/// change. `index` in the error names the attachment a refused value came from.
pub(crate) fn serialise(metadata: &Metadata) -> Result<Vec<u8>, CreateError> {
    let mut writer = Writer {
        out: String::from(DECLARATION),
    };
    writer.out.push_str(&format!(
        "<{PREFIX}:{root} xmlns:{PREFIX}=\"{TARGET_NAMESPACE}\">",
        root = MetadataField::Kuldemeny.local_name()
    ));
    writer.header(metadata)?;
    writer.marker(metadata.receipt_present, MetadataField::Erkeztetes);
    writer.marker(metadata.openings_present, MetadataField::Bontasok);
    writer.dispatches(metadata)?;
    writer.marker(metadata.return_receipt_present, MetadataField::Tertiveveny);
    writer.close(MetadataField::Kuldemeny);
    Ok(writer.out.into_bytes())
}

/// The document being assembled, one element at a time.
struct Writer {
    out: String,
}

impl Writer {
    /// `FEJRESZ`, in the M3 order.
    fn header(&mut self, metadata: &Metadata) -> Result<(), CreateError> {
        let header = &metadata.header;
        self.open(MetadataField::Fejresz);
        self.leaf(MetadataField::KrxVerzioszam, &header.version, None)?;
        self.raw_leaf(
            MetadataField::ForrasrendszerAzonosito,
            header.source_system.as_str(),
        );
        self.leaf(
            MetadataField::KuldemenyAzonosito,
            &header.consignment_id,
            None,
        )?;
        self.leaf(
            MetadataField::KuldemenyLetrehozasanakIdeje,
            &header.created_at_text,
            None,
        )?;
        self.raw_leaf(
            MetadataField::KuldemenyTipus,
            header.consignment_kind.as_str(),
        );
        if header.test_present {
            // M11 records one official example omitting TESZT, so an absent
            // element is written as absent rather than defaulted into presence.
            self.raw_leaf(
                MetadataField::Teszt,
                if header.test { "true" } else { "false" },
            );
        }
        self.optional(MetadataField::Vonalkod, header.barcode.as_deref())?;
        self.optional(
            MetadataField::KuldemenyHivatkozasiAzonosito,
            header.reference_id.as_deref(),
        )?;
        self.optional(MetadataField::Hibakod, header.error_code.as_deref())?;
        self.optional(MetadataField::KuldemenyMegjegyzes, header.note.as_deref())?;
        self.close(MetadataField::Fejresz);
        Ok(())
    }

    /// `EXPEDIALASOK`, or nothing at all when the document carries no dispatch.
    fn dispatches(&mut self, metadata: &Metadata) -> Result<(), CreateError> {
        if metadata.dispatches.is_empty() {
            return Ok(());
        }
        self.open(MetadataField::Expedialasok);
        for dispatch in &metadata.dispatches {
            self.open(MetadataField::Expedialas);
            if let Some(count) = dispatch.declared_attachment_count {
                self.raw_leaf(MetadataField::MellekletekSzama, &count.to_string());
            }
            self.open(MetadataField::Mellekletek);
            for (index, attachment) in dispatch.attachments.iter().enumerate() {
                self.attachment(attachment, u32::try_from(index).ok())?;
            }
            self.close(MetadataField::Mellekletek);
            if dispatch.handling_instructions_unqualified {
                // M8: this one element is declared unqualified, so it is
                // written with no prefix and inherits no default namespace.
                let name = MetadataField::KezelesiUtasitasok.local_name();
                self.out.push_str(&format!("<{name}></{name}>"));
            }
            self.close(MetadataField::Expedialas);
        }
        self.close(MetadataField::Expedialasok);
        Ok(())
    }

    /// One `MELLEKLET`, in the M5 order.
    fn attachment(
        &mut self,
        attachment: &AttachmentReference,
        index: Option<u32>,
    ) -> Result<(), CreateError> {
        self.open(MetadataField::Melleklet);
        if let Some(description) = &attachment.description {
            self.leaf(MetadataField::MellekletLeirasa, description, index)?;
        }
        self.raw_leaf(
            MetadataField::CsatolmanySzama,
            &attachment.number.to_string(),
        );
        self.leaf(MetadataField::FajlNev, &attachment.file_name, index)?;
        self.leaf(MetadataField::Meret, &attachment.size_text, index)?;
        self.leaf(MetadataField::Elhelyezkedes, &attachment.location, index)?;
        self.optional_at(
            MetadataField::Mennyiseg,
            attachment.quantity.as_deref(),
            index,
        )?;
        self.optional_at(
            MetadataField::MennyisegiEgyseg,
            attachment.quantity_unit.as_deref(),
            index,
        )?;
        self.close(MetadataField::Melleklet);
        Ok(())
    }

    /// An empty element standing for a block this crate does not interpret.
    ///
    /// `ERKEZTETES`, `BONTASOK` and `TERTIVEVENY` are recorded by the reader as
    /// present or absent and never read further, so presence is all the writer
    /// can honestly reproduce.
    fn marker(&mut self, present: bool, field: MetadataField) {
        if present {
            self.open(field);
            self.close(field);
        }
    }

    /// A qualified opening tag.
    fn open(&mut self, field: MetadataField) {
        self.out
            .push_str(&format!("<{PREFIX}:{}>", field.local_name()));
    }

    /// A qualified closing tag.
    fn close(&mut self, field: MetadataField) {
        self.out
            .push_str(&format!("</{PREFIX}:{}>", field.local_name()));
    }

    /// A leaf whose text is caller content, escaped and checked first.
    fn leaf(
        &mut self,
        field: MetadataField,
        value: &str,
        index: Option<u32>,
    ) -> Result<(), CreateError> {
        let escaped = escape(value, index)?;
        self.out.push_str(&format!(
            "<{PREFIX}:{name}>{escaped}</{PREFIX}:{name}>",
            name = field.local_name()
        ));
        Ok(())
    }

    /// A leaf whose text this crate produced: an enumeration token or a number.
    fn raw_leaf(&mut self, field: MetadataField, value: &str) {
        self.out.push_str(&format!(
            "<{PREFIX}:{name}>{value}</{PREFIX}:{name}>",
            name = field.local_name()
        ));
    }

    /// A leaf written only when the value is present.
    fn optional(&mut self, field: MetadataField, value: Option<&str>) -> Result<(), CreateError> {
        self.optional_at(field, value, None)
    }

    /// The same, scoped to the attachment the value came from.
    fn optional_at(
        &mut self,
        field: MetadataField,
        value: Option<&str>,
        index: Option<u32>,
    ) -> Result<(), CreateError> {
        match value {
            Some(value) => self.leaf(field, value, index),
            None => Ok(()),
        }
    }
}

/// Escape one text value, refusing what XML 1.0 or the reader cannot carry.
///
/// Element content only: `"` and `'` are deliberately not escaped, so the
/// result is not safe inside an attribute value. The serialiser writes exactly
/// one attribute, the fixed namespace declaration, and never a caller's text.
fn escape(value: &str, index: Option<u32>) -> Result<String, CreateError> {
    let invalid = |kind| CreateError::Invalid { kind, index };
    if value != value.trim() {
        return Err(invalid(InvalidKind::UntrimmedText));
    }
    let mut out = String::with_capacity(value.len());
    for character in value.chars() {
        if !is_writable(character) {
            return Err(invalid(InvalidKind::Text));
        }
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            other => out.push(other),
        }
    }
    Ok(out)
}

/// Whether a character can be written and read back unchanged.
///
/// XML 1.0 admits tab, line feed and carriage return among the C0 controls and
/// nothing else below `U+0020`, and admits no non-character. Carriage return is
/// refused as well: a parser rewrites it during line-ending normalisation, so a
/// document holding one would not read back as the value that was written.
const fn is_writable(character: char) -> bool {
    match character {
        '\t' | '\n' => true,
        '\u{0}'..='\u{1f}' => false,
        '\u{fffe}' | '\u{ffff}' => false,
        _ => true,
    }
}
