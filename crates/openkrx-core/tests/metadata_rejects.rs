//! Refused metadata documents: prohibited XML features, grammar violations and
//! every limit boundary. Each test asserts a stable code, so one rejection
//! category cannot silently become another.

mod support;

use openkrx_core::metadata::{self, MetadataField, MetadataLimits};
use support::meta::Document;

fn code_of(document: &[u8]) -> &'static str {
    metadata::parse(document, &MetadataLimits::DEFAULT)
        .expect_err("document should be refused")
        .code()
}

fn code_with(document: &[u8], limits: &MetadataLimits) -> &'static str {
    metadata::parse(document, limits)
        .expect_err("document should be refused")
        .code()
}

fn accepted_with(document: &[u8], limits: &MetadataLimits) -> bool {
    metadata::parse(document, limits).is_ok()
}

// ---------------------------------------------------------------- XML features

#[test]
fn a_doctype_declaration_is_refused_before_anything_is_declared() {
    let document = format!(
        "<!DOCTYPE KULDEMENY><KULDEMENY xmlns=\"{}\"/>",
        metadata::TARGET_NAMESPACE
    );
    assert_eq!(code_of(document.as_bytes()), "metadata.unsupported.dtd");
}

#[test]
fn an_external_entity_shaped_doctype_is_refused_as_a_doctype() {
    // The classic XXE shape never reaches entity resolution: the declaration
    // itself is refused, so no system identifier is ever looked at.
    let document = "<!DOCTYPE KULDEMENY [<!ENTITY x SYSTEM \"file:///etc/passwd\">]>\
                    <KULDEMENY><FEJRESZ>&x;</FEJRESZ></KULDEMENY>";
    assert_eq!(code_of(document.as_bytes()), "metadata.unsupported.dtd");
}

#[test]
fn an_undeclared_entity_reference_is_refused_in_text() {
    let document = Document {
        consignment_id: Some("&payload;".to_owned()),
        ..Document::default()
    };
    assert_eq!(code_of(&document.bytes()), "metadata.malformed.entity");
}

#[test]
fn an_undeclared_entity_reference_is_refused_in_an_attribute_value() {
    let xml = Document::default()
        .xml()
        .replace("<ns2:FEJRESZ>", "<ns2:FEJRESZ attr=\"&payload;\">");
    assert_eq!(code_of(xml.as_bytes()), "metadata.malformed.entity");
}

#[test]
fn a_character_reference_to_a_forbidden_code_point_is_refused() {
    let document = Document {
        consignment_id: Some("&#0;".to_owned()),
        ..Document::default()
    };
    assert_eq!(code_of(&document.bytes()), "metadata.malformed.entity");
}

#[test]
fn a_processing_instruction_other_than_the_declaration_is_refused() {
    let xml = Document::default()
        .xml()
        .replace("<ns2:FEJRESZ>", "<?run something?><ns2:FEJRESZ>");
    assert_eq!(
        code_of(xml.as_bytes()),
        "metadata.unsupported.processing_instruction"
    );
}

#[test]
fn a_non_utf8_encoding_declaration_is_refused_rather_than_guessed() {
    let xml = Document::default()
        .xml()
        .replace("encoding=\"UTF-8\"", "encoding=\"ISO-8859-2\"");
    assert_eq!(code_of(xml.as_bytes()), "metadata.unsupported.encoding");
}

#[test]
fn bytes_that_are_not_utf8_are_refused_as_an_encoding_problem() {
    let mut document = Document::default().bytes();
    let position = document
        .windows(7)
        .position(|window| window == b"SYN-000")
        .expect("marker present");
    document[position] = 0xff;
    assert_eq!(code_of(&document), "metadata.malformed.encoding");
}

#[test]
fn an_unbound_namespace_prefix_is_refused() {
    let xml = Document::default()
        .xml()
        .replace("<ns2:FEJRESZ>", "<nope:FEJRESZ>")
        .replace("</ns2:FEJRESZ>", "</nope:FEJRESZ>");
    assert_eq!(code_of(xml.as_bytes()), "metadata.malformed.unbound_prefix");
}

#[test]
fn a_repeated_attribute_is_refused() {
    let xml = Document::default()
        .xml()
        .replace("<ns2:FEJRESZ>", "<ns2:FEJRESZ a=\"1\" a=\"2\">");
    assert_eq!(code_of(xml.as_bytes()), "metadata.malformed.attribute");
}

#[test]
fn an_unclosed_element_is_refused_as_a_syntax_problem() {
    let xml = Document::default().xml().replace("</ns2:KULDEMENY>", "");
    assert_eq!(code_of(xml.as_bytes()), "metadata.malformed.syntax");
}

#[test]
fn an_empty_document_is_refused_as_a_missing_root() {
    assert_eq!(code_of(b""), "metadata.malformed.missing_element");
}

// ------------------------------------------------------------------- grammar

#[test]
fn a_root_in_another_namespace_is_unsupported_not_malformed() {
    let document = Document {
        namespace: "urn:example:other".to_owned(),
        ..Document::default()
    };
    assert_eq!(code_of(&document.bytes()), "metadata.unsupported.namespace");
}

#[test]
fn an_unqualified_root_is_unsupported_because_the_schema_is_qualified() {
    // M1 fixes elementFormDefault="qualified", so a root in no namespace is
    // a different document shape, not a malformed KULDEMENY.
    assert_eq!(code_of(b"<KULDEMENY/>"), "metadata.unsupported.namespace");
}

#[test]
fn a_wrong_root_element_in_the_right_namespace_is_malformed() {
    let xml = Document::default()
        .xml()
        .replace("ns2:KULDEMENY", "ns2:VALAMI");
    assert_eq!(code_of(xml.as_bytes()), "metadata.malformed.root_element");
}

#[test]
fn a_missing_required_header_element_is_reported_with_its_field() {
    let document = Document {
        consignment_id: None,
        ..Document::default()
    };
    let error = metadata::parse(&document.bytes(), &MetadataLimits::DEFAULT)
        .expect_err("document should be refused");
    assert_eq!(error.code(), "metadata.malformed.missing_element");
    assert_eq!(error.field(), Some(MetadataField::KuldemenyAzonosito));
}

#[test]
fn a_document_without_a_header_at_all_is_refused() {
    let xml = format!(
        "<KULDEMENY xmlns=\"{}\"></KULDEMENY>",
        metadata::TARGET_NAMESPACE
    );
    assert_eq!(
        code_of(xml.as_bytes()),
        "metadata.malformed.missing_element"
    );
}

#[test]
fn a_value_outside_its_enumeration_is_refused() {
    for document in [
        Document {
            source_system: Some("NOVA2".to_owned()),
            ..Document::default()
        },
        Document {
            consignment_kind: Some("kuldemeny".to_owned()),
            ..Document::default()
        },
    ] {
        assert_eq!(code_of(&document.bytes()), "metadata.malformed.enumeration");
    }
}

#[test]
fn a_non_boolean_teszt_is_refused() {
    let document = Document {
        test: Some("igen".to_owned()),
        ..Document::default()
    };
    assert_eq!(code_of(&document.bytes()), "metadata.malformed.boolean");
}

#[test]
fn a_non_integer_attachment_number_is_refused() {
    let mut attachment = document_attachment();
    attachment.number = "1.5".to_owned();
    let document = Document {
        attachments: vec![attachment],
        ..Document::default()
    };
    assert_eq!(code_of(&document.bytes()), "metadata.malformed.integer");
}

#[test]
fn a_non_integer_declared_count_is_refused() {
    let document = Document {
        declared_count: Some("many".to_owned()),
        ..Document::default()
    };
    assert_eq!(code_of(&document.bytes()), "metadata.malformed.integer");
}

#[test]
fn a_repeated_header_element_is_refused() {
    let xml = Document::default().xml().replace(
        "</ns2:FEJRESZ>",
        "<ns2:KRX_VERZIOSZAM>v0.8</ns2:KRX_VERZIOSZAM></ns2:FEJRESZ>",
    );
    assert_eq!(
        code_of(xml.as_bytes()),
        "metadata.malformed.duplicate_element"
    );
}

#[test]
fn a_repeated_declared_count_is_refused() {
    let xml = Document::default().xml().replace(
        "<ns2:MELLEKLETEK>",
        "<ns2:MELLEKLETEK_SZAMA>2</ns2:MELLEKLETEK_SZAMA><ns2:MELLEKLETEK>",
    );
    assert_eq!(
        code_of(xml.as_bytes()),
        "metadata.malformed.duplicate_element"
    );
}

#[test]
fn a_repeated_sibling_block_is_refused() {
    let xml = Document::default()
        .xml()
        .replace("</ns2:KULDEMENY>", "<ns2:FEJRESZ/></ns2:KULDEMENY>");
    assert_eq!(
        code_of(xml.as_bytes()),
        "metadata.malformed.duplicate_element"
    );
}

#[test]
fn sibling_blocks_out_of_the_documented_order_are_refused() {
    // M2 is the one rule that states an order, so it is the one enforced.
    let xml = Document::default()
        .xml()
        .replace("</ns2:KULDEMENY>", "<ns2:ERKEZTETES/></ns2:KULDEMENY>");
    assert_eq!(code_of(xml.as_bytes()), "metadata.malformed.element_order");
}

#[test]
fn a_child_element_inside_a_leaf_field_is_refused() {
    let xml = Document::default().xml().replace(
        "<ns2:KRX_VERZIOSZAM>v0.9</ns2:KRX_VERZIOSZAM>",
        "<ns2:KRX_VERZIOSZAM><ns2:X/></ns2:KRX_VERZIOSZAM>",
    );
    assert_eq!(
        code_of(xml.as_bytes()),
        "metadata.malformed.unexpected_child"
    );
}

// -------------------------------------------------------------------- limits

#[test]
fn the_document_size_limit_holds_at_its_boundary() {
    let document = Document::default().bytes();
    let mut limits = MetadataLimits::DEFAULT;
    limits.max_document_bytes = document.len() as u64;
    assert!(accepted_with(&document, &limits));
    limits.max_document_bytes -= 1;
    assert_eq!(
        code_with(&document, &limits),
        "metadata.over_limit.document_bytes"
    );
}

#[test]
fn the_depth_limit_holds_at_its_boundary() {
    // KULDEMENY / EXPEDIALASOK / EXPEDIALAS / MELLEKLETEK / MELLEKLET / leaf.
    let document = Document::default().bytes();
    let mut limits = MetadataLimits::DEFAULT;
    limits.max_depth = 6;
    assert!(accepted_with(&document, &limits));
    limits.max_depth = 5;
    assert_eq!(code_with(&document, &limits), "metadata.over_limit.depth");
}

#[test]
fn the_element_count_limit_holds_at_its_boundary() {
    let document = Document::default().bytes();
    let counted = count_elements(&document);
    let mut limits = MetadataLimits::DEFAULT;
    limits.max_elements = counted;
    assert!(accepted_with(&document, &limits));
    limits.max_elements = counted - 1;
    assert_eq!(
        code_with(&document, &limits),
        "metadata.over_limit.elements"
    );
}

#[test]
fn the_attribute_count_limit_holds_at_its_boundary() {
    let attributes: String = (0..4).map(|index| format!(" a{index}=\"v\"")).collect();
    let xml = Document::default()
        .xml()
        .replace("<ns2:FEJRESZ>", &format!("<ns2:FEJRESZ{attributes}>"));
    let mut limits = MetadataLimits::DEFAULT;
    limits.max_attributes_per_element = 4;
    assert!(accepted_with(xml.as_bytes(), &limits));
    limits.max_attributes_per_element = 3;
    assert_eq!(
        code_with(xml.as_bytes(), &limits),
        "metadata.over_limit.attributes_per_element"
    );
}

#[test]
fn the_text_limit_holds_at_its_boundary() {
    let document = Document::default().bytes();
    let text = count_text_bytes(&document);
    let mut limits = MetadataLimits::DEFAULT;
    limits.max_text_bytes = text;
    assert!(accepted_with(&document, &limits));
    limits.max_text_bytes = text - 1;
    assert_eq!(
        code_with(&document, &limits),
        "metadata.over_limit.text_bytes"
    );
}

#[test]
fn a_deeply_nested_unknown_subtree_still_meets_the_depth_limit() {
    let nesting = 40;
    let body = format!(
        "<ns2:ISMERETLEN>{}{}</ns2:ISMERETLEN>",
        "<ns2:X>".repeat(nesting),
        "</ns2:X>".repeat(nesting)
    );
    let document = Document {
        extra_body: body,
        ..Document::default()
    };
    assert_eq!(code_of(&document.bytes()), "metadata.over_limit.depth");
}

// ------------------------------------------------------------------ diagnostics

#[test]
fn error_display_carries_codes_and_numbers_but_no_document_content() {
    let document = Document {
        consignment_id: Some("SECRET-CONTENT".to_owned()),
        source_system: Some("SECRET-SYSTEM".to_owned()),
        ..Document::default()
    };
    let error = metadata::parse(&document.bytes(), &MetadataLimits::DEFAULT)
        .expect_err("document should be refused");
    let text = error.to_string();
    assert_eq!(text, "metadata.malformed.enumeration");
    assert!(!text.contains("SECRET"));
    assert!(!text.contains("FORRASRENDSZER"));

    let mut limits = MetadataLimits::DEFAULT;
    limits.max_elements = 1;
    let limited = metadata::parse(&document.bytes(), &limits).expect_err("refused");
    assert_eq!(
        limited.to_string(),
        "metadata.over_limit.elements (limit 1, observed 2)"
    );
    assert_eq!(limited.field(), None);
}

#[test]
fn no_truncation_of_a_valid_document_is_ever_accepted_or_panics() {
    let document = Document::default().bytes();
    for length in 0..document.len() {
        assert!(
            metadata::parse(&document[..length], &MetadataLimits::DEFAULT).is_err(),
            "prefix of {length} bytes was accepted"
        );
    }
}

#[test]
fn single_byte_mutations_never_panic() {
    let document = Document::default().bytes();
    for position in 0..document.len() {
        for replacement in [0x00_u8, 0x26, 0x3c, 0x3e, 0x2f, 0x80, 0xff] {
            let mut mutated = document.clone();
            mutated[position] = replacement;
            let _ = metadata::parse(&mutated, &MetadataLimits::DEFAULT);
        }
    }
}

#[test]
fn a_truncated_prefix_of_every_refused_document_also_never_panics() {
    let documents = [
        Document {
            test: Some("igen".to_owned()),
            ..Document::default()
        }
        .bytes(),
        Document {
            namespace: "urn:example:other".to_owned(),
            ..Document::default()
        }
        .bytes(),
        b"<!DOCTYPE a><a/>".to_vec(),
    ];
    for document in documents {
        for length in 0..=document.len() {
            let _ = metadata::parse(&document[..length], &MetadataLimits::DEFAULT);
        }
    }
}

/// A complete reference, for tests that then break exactly one field.
fn document_attachment() -> support::meta::Attachment {
    support::meta::Attachment::new(1, "synthetic.pdf", "KRX/OCD/Payload/ID-1")
}

/// Count start tags, independently of the crate under test.
fn count_elements(document: &[u8]) -> u32 {
    let text = core::str::from_utf8(document).expect("synthetic document is UTF-8");
    u32::try_from(
        text.match_indices('<')
            .filter(|(index, _)| {
                !text[index + 1..].starts_with('/')
                    && !text[index + 1..].starts_with('?')
                    && !text[index + 1..].starts_with('!')
            })
            .count(),
    )
    .expect("test document is small")
}

/// Count character data between tags, independently of the crate under test.
fn count_text_bytes(document: &[u8]) -> u64 {
    let text = core::str::from_utf8(document).expect("synthetic document is UTF-8");
    let mut total = 0_u64;
    let mut inside = false;
    for character in text.chars() {
        match character {
            '<' => inside = true,
            '>' => inside = false,
            _ if !inside => total += character.len_utf8() as u64,
            _ => {}
        }
    }
    total
}
