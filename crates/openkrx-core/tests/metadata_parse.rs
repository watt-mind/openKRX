//! Accepted metadata documents: what the parser reports and what it declines
//! to conclude.

mod support;

use openkrx_core::metadata::{self, ConsignmentKind, MetadataLimits, SourceSystem};
use support::meta::{Attachment, Document, NAMESPACE};

fn read(document: &Document) -> metadata::Metadata {
    metadata::parse(&document.bytes(), &MetadataLimits::DEFAULT)
        .expect("document should be accepted")
}

#[test]
fn a_prefixed_document_reports_its_header_verbatim() {
    let parsed = read(&Document::default());
    assert_eq!(parsed.header.version, "v0.9");
    assert_eq!(parsed.header.source_system, SourceSystem::Ker);
    assert_eq!(parsed.header.consignment_id, "SYN-0001");
    assert_eq!(
        parsed.header.created_at_text,
        "2026-01-02T03:04:05.000+01:00"
    );
    assert_eq!(parsed.header.consignment_kind, ConsignmentKind::Kuldemeny);
    assert!(!parsed.header.test);
    assert!(parsed.header.test_present);
    assert_eq!(parsed.unknown_elements, 0);
}

#[test]
fn a_default_namespace_binding_parses_the_same_as_a_prefixed_one() {
    let mut prefixed = Document::default();
    prefixed.prefix.clear();
    assert_eq!(read(&prefixed).header, read(&Document::default()).header);
}

#[test]
fn the_target_namespace_constant_matches_the_documented_one() {
    assert_eq!(metadata::TARGET_NAMESPACE, NAMESPACE);
}

#[test]
fn an_absent_teszt_is_recorded_rather_than_defaulted_silently() {
    // M11: one official example omits an element the schema requires.
    let document = Document {
        test: None,
        ..Document::default()
    };
    let parsed = read(&document);
    assert!(!parsed.header.test);
    assert!(!parsed.header.test_present);
}

#[test]
fn teszt_accepts_both_boolean_lexical_forms() {
    for (written, expected) in [("true", true), ("1", true), ("false", false), ("0", false)] {
        let document = Document {
            test: Some(written.to_owned()),
            ..Document::default()
        };
        assert_eq!(read(&document).header.test, expected, "for {written}");
    }
}

#[test]
fn every_enumeration_token_the_schema_lists_is_accepted() {
    for token in ["NOVA", "KIR3", "KER", "POSTA", "IMAP"] {
        let document = Document {
            source_system: Some(token.to_owned()),
            ..Document::default()
        };
        assert_eq!(read(&document).header.source_system.as_str(), token);
    }
    for token in [
        "KULDEMENY",
        "NYUGTA",
        "EXPEDIALAS",
        "TERTIVEVENY",
        "HIBAJELZES",
    ] {
        let document = Document {
            consignment_kind: Some(token.to_owned()),
            ..Document::default()
        };
        assert_eq!(read(&document).header.consignment_kind.as_str(), token);
    }
}

#[test]
fn optional_header_elements_are_absent_when_the_document_omits_them() {
    let parsed = read(&Document::default());
    assert_eq!(parsed.header.barcode, None);
    assert_eq!(parsed.header.reference_id, None);
    assert_eq!(parsed.header.error_code, None);
    assert_eq!(parsed.header.note, None);
}

#[test]
fn optional_header_elements_are_read_when_present() {
    let document = Document {
        extra_body: String::new(),
        ..Document::default()
    };
    let xml = document.xml().replace(
        "</ns2:FEJRESZ>",
        "<ns2:VONALKOD>SYN-BC</ns2:VONALKOD>\
         <ns2:KULDEMENY_HIVATKOZASI_AZONOSITO>SYN-REF</ns2:KULDEMENY_HIVATKOZASI_AZONOSITO>\
         <ns2:HIBAKOD>SYN-ERR</ns2:HIBAKOD>\
         <ns2:KULDEMENY_MEGJEGYZES>synthetic note</ns2:KULDEMENY_MEGJEGYZES>\
         </ns2:FEJRESZ>",
    );
    let parsed = metadata::parse(xml.as_bytes(), &MetadataLimits::DEFAULT).expect("accepted");
    assert_eq!(parsed.header.barcode.as_deref(), Some("SYN-BC"));
    assert_eq!(parsed.header.reference_id.as_deref(), Some("SYN-REF"));
    assert_eq!(parsed.header.error_code.as_deref(), Some("SYN-ERR"));
    assert_eq!(parsed.header.note.as_deref(), Some("synthetic note"));
}

#[test]
fn an_attachment_reference_keeps_its_declared_size_text_and_joined_path() {
    let parsed = read(&Document::default());
    let attachment = parsed.attachments().next().expect("one reference");
    assert_eq!(attachment.number, 1);
    assert_eq!(attachment.file_name, "synthetic.pdf");
    assert_eq!(attachment.size_text, "12.5");
    assert_eq!(attachment.size_value, Some(12.5));
    assert_eq!(attachment.location, "KRX/OCD/Payload/ID-1");
    assert_eq!(
        attachment.declared_path(),
        "KRX/OCD/Payload/ID-1/synthetic.pdf"
    );
}

#[test]
fn a_trailing_separator_on_the_location_does_not_double_the_join() {
    let mut attachment = Attachment::new(1, "synthetic.pdf", "KRX/OCD/Payload/ID-1/");
    attachment.size = "1".to_owned();
    let document = Document {
        attachments: vec![attachment],
        ..Document::default()
    };
    let parsed = read(&document);
    assert_eq!(
        parsed.attachments().next().expect("one").declared_path(),
        "KRX/OCD/Payload/ID-1/synthetic.pdf"
    );
}

#[test]
fn an_empty_location_leaves_the_file_name_as_the_whole_path() {
    let mut attachment = Attachment::new(1, "synthetic.pdf", "");
    attachment.size = "1".to_owned();
    let document = Document {
        attachments: vec![attachment],
        ..Document::default()
    };
    let parsed = read(&document);
    assert_eq!(
        parsed.attachments().next().expect("one").declared_path(),
        "synthetic.pdf"
    );
}

#[test]
fn a_non_numeric_declared_size_is_kept_as_text_rather_than_rejected() {
    // M11 and M13: an official example writes MERET as a string, and the unit
    // is unresolved, so the reader keeps the text and offers no number.
    let mut attachment = Attachment::new(1, "synthetic.pdf", "KRX/OCD/Payload/ID-1");
    attachment.size = "60110 bytes".to_owned();
    let document = Document {
        attachments: vec![attachment],
        ..Document::default()
    };
    let parsed = read(&document);
    let attachment = parsed.attachments().next().expect("one reference");
    assert_eq!(attachment.size_text, "60110 bytes");
    assert_eq!(attachment.size_value, None);
}

#[test]
fn an_infinite_declared_size_is_not_offered_as_a_number() {
    let mut attachment = Attachment::new(1, "synthetic.pdf", "KRX/OCD/Payload/ID-1");
    attachment.size = "inf".to_owned();
    let document = Document {
        attachments: vec![attachment],
        ..Document::default()
    };
    assert_eq!(
        read(&document)
            .attachments()
            .next()
            .expect("one")
            .size_value,
        None
    );
}

#[test]
fn an_absent_attachment_description_is_recorded_rather_than_rejected() {
    // M11 again: the same official example omits MELLEKLET_LEIRASA.
    let mut attachment = Attachment::new(1, "synthetic.pdf", "KRX/OCD/Payload/ID-1");
    attachment.description = None;
    let document = Document {
        attachments: vec![attachment],
        ..Document::default()
    };
    assert_eq!(
        read(&document)
            .attachments()
            .next()
            .expect("one")
            .description,
        None
    );
}

#[test]
fn optional_attachment_elements_are_read_when_present() {
    let xml = Document::default().xml().replace(
        "</ns2:MELLEKLET>",
        "<ns2:MENNYISEG>3</ns2:MENNYISEG>\
         <ns2:MENNYISEGI_EGYSEG>db</ns2:MENNYISEGI_EGYSEG></ns2:MELLEKLET>",
    );
    let parsed = metadata::parse(xml.as_bytes(), &MetadataLimits::DEFAULT).expect("accepted");
    let attachment = parsed.attachments().next().expect("one reference");
    assert_eq!(attachment.quantity.as_deref(), Some("3"));
    assert_eq!(attachment.quantity_unit.as_deref(), Some("db"));
}

#[test]
fn an_unqualified_handling_instruction_is_observed_as_such() {
    // M8: the schema declares this one element unqualified.
    let parsed = read(&Document::default());
    assert!(parsed.dispatches[0].handling_instructions_unqualified);
    assert_eq!(parsed.dispatches[0].declared_attachment_count, Some(1));
}

#[test]
fn a_qualified_handling_instruction_is_an_unknown_element_not_the_real_one() {
    let xml = Document::default().xml().replace(
        "<KEZELESI_UTASITASOK>synthetic instruction</KEZELESI_UTASITASOK>",
        "<ns2:KEZELESI_UTASITASOK>synthetic</ns2:KEZELESI_UTASITASOK>",
    );
    let parsed = metadata::parse(xml.as_bytes(), &MetadataLimits::DEFAULT).expect("accepted");
    assert!(!parsed.dispatches[0].handling_instructions_unqualified);
    assert_eq!(parsed.unknown_elements, 1);
}

#[test]
fn the_optional_sibling_blocks_are_observed_without_being_interpreted() {
    let document = Document {
        extra_body: "<ns2:ERKEZTETES><ns2:B/></ns2:ERKEZTETES>\
                     <ns2:BONTASOK><ns2:C/></ns2:BONTASOK>"
            .to_owned(),
        ..Document::default()
    };
    let xml = document
        .xml()
        .replace("</ns2:KULDEMENY>", "<ns2:TERTIVEVENY/></ns2:KULDEMENY>");
    let parsed = metadata::parse(xml.as_bytes(), &MetadataLimits::DEFAULT).expect("accepted");
    assert!(parsed.receipt_present);
    assert!(parsed.openings_present);
    assert!(parsed.return_receipt_present);
    // Their content is skipped wholesale, so nothing inside them is counted.
    assert_eq!(parsed.unknown_elements, 0);
}

#[test]
fn an_unknown_element_is_counted_rather_than_rejected() {
    // A9: a container may carry further descriptive information.
    let document = Document {
        extra_body: "<ns2:ISMERETLEN>x</ns2:ISMERETLEN>\
                     <other:X xmlns:other=\"urn:example:other\">y</other:X>"
            .to_owned(),
        ..Document::default()
    };
    assert_eq!(read(&document).unknown_elements, 2);
}

#[test]
fn predefined_entities_and_character_references_are_resolved_in_text() {
    let document = Document {
        consignment_id: Some("a&amp;b&lt;c&gt;d&apos;e&quot;f&#65;&#x42;".to_owned()),
        ..Document::default()
    };
    assert_eq!(read(&document).header.consignment_id, "a&b<c>d'e\"fAB");
}

#[test]
fn cdata_and_comments_do_not_change_a_value() {
    let xml = Document::default().xml().replace(
        "<ns2:KULDEMENY_AZONOSITO>SYN-0001</ns2:KULDEMENY_AZONOSITO>",
        "<ns2:KULDEMENY_AZONOSITO><![CDATA[SYN]]><!-- c -->-0001</ns2:KULDEMENY_AZONOSITO>",
    );
    let parsed = metadata::parse(xml.as_bytes(), &MetadataLimits::DEFAULT).expect("accepted");
    assert_eq!(parsed.header.consignment_id, "SYN-0001");
}

#[test]
fn a_document_without_an_xml_declaration_parses_identically() {
    let document = Document {
        declaration: false,
        ..Document::default()
    };
    assert_eq!(read(&document).header, read(&Document::default()).header);
}

#[test]
fn a_header_only_document_lists_no_dispatch_and_no_attachment() {
    let parsed = read(&Document::header_only());
    assert!(parsed.dispatches.is_empty());
    assert_eq!(parsed.attachment_count(), 0);
    assert_eq!(parsed.attachments().count(), 0);
}

#[test]
fn header_elements_may_appear_in_any_order() {
    // Only M2 states an order, and it states it for the KULDEMENY children.
    let xml = Document::default().xml().replace(
        "<ns2:KRX_VERZIOSZAM>v0.9</ns2:KRX_VERZIOSZAM>\
         <ns2:FORRASRENDSZER_AZONOSITO>KER</ns2:FORRASRENDSZER_AZONOSITO>",
        "<ns2:FORRASRENDSZER_AZONOSITO>KER</ns2:FORRASRENDSZER_AZONOSITO>\
         <ns2:KRX_VERZIOSZAM>v0.9</ns2:KRX_VERZIOSZAM>",
    );
    assert!(metadata::parse(xml.as_bytes(), &MetadataLimits::DEFAULT).is_ok());
}

#[test]
fn the_default_metadata_limits_are_the_documented_ones() {
    let limits = MetadataLimits::default();
    assert_eq!(limits, MetadataLimits::DEFAULT);
    assert_eq!(limits.max_document_bytes, 32 * 1024 * 1024);
    assert_eq!(limits.max_depth, 32);
    assert_eq!(limits.max_elements, 10_000);
    assert_eq!(limits.max_attributes_per_element, 32);
    assert_eq!(limits.max_text_bytes, 1024 * 1024);
}

#[test]
fn each_optional_sibling_block_is_observed_on_its_own_and_not_for_its_neighbours() {
    // The three presence flags are set independently. A document carrying all
    // three cannot tell them apart, so each is presented alone here: the one
    // block present must be the only flag set.
    let cases = [
        ("<ns2:ERKEZTETES/>", [true, false, false]),
        ("<ns2:BONTASOK/>", [false, true, false]),
        ("<ns2:TERTIVEVENY/>", [false, false, true]),
    ];
    for (element, expected) in cases {
        // TERTIVEVENY follows the dispatch blocks in the schema sequence, so
        // it is appended rather than inserted with the other two.
        let xml = if element == "<ns2:TERTIVEVENY/>" {
            Document::default()
                .xml()
                .replace("</ns2:KULDEMENY>", "<ns2:TERTIVEVENY/></ns2:KULDEMENY>")
        } else {
            Document {
                extra_body: element.to_owned(),
                ..Document::default()
            }
            .xml()
        };
        let parsed = metadata::parse(xml.as_bytes(), &MetadataLimits::DEFAULT).expect("accepted");
        assert_eq!(
            [
                parsed.receipt_present,
                parsed.openings_present,
                parsed.return_receipt_present,
            ],
            expected,
            "{element} sets its own flag and no other"
        );
    }

    // And a document carrying none of them sets none.
    let parsed = metadata::parse(
        Document::default().xml().as_bytes(),
        &MetadataLimits::DEFAULT,
    )
    .expect("accepted");
    assert!(!parsed.receipt_present);
    assert!(!parsed.openings_present);
    assert!(!parsed.return_receipt_present);
}
