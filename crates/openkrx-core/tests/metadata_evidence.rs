//! Independent evidence: a synthetic re-expression of the documented sample
//! structure.
//!
//! `docs/profile.md` describes two official `KULDEMENY_META.xml` samples, one
//! for a successful and one for an unsuccessful acceptance, and records two
//! observations about them: rule M9, that they are serialised with an XML
//! declaration and bind the target namespace to the `ns2` prefix; and rule M10,
//! that `ELHELYEZKEDES` names the payload directory while the file name sits in
//! `FAJL_NEV`, so a reference is the join of the two.
//!
//! The documents below reproduce that **structure** with values written from
//! scratch for this repository. No identifier, description, timestamp, barcode
//! or other text is copied from an official sample: `docs/profile.md` records
//! that the posta.hu material carries no redistribution licence, and
//! `tests/fixtures/README.md` records that these are synthetic re-expressions.
//!
//! This is evidence of schema shape only. `docs/profile.md#conformance-evidence`
//! records that no public sample archive and no independent conformance corpus
//! exist, so agreement with a real service remains unverified.

mod support;

use openkrx_core::metadata::{self, ConsignmentKind, MetadataLimits, SourceSystem};
use openkrx_core::profile::{self, CheckId, CheckOutcome, ReferenceResolution, RuleId};
use openkrx_core::{Limits, archive};
use support::meta::{MARKER_CONTENT, METADATA_FILE, NAMESPACE};
use support::{Archive, Entry};

/// The shape both samples share, per M9: declaration, `ns2` prefix, header.
fn sample(kind: &str, tail: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <ns2:KULDEMENY xmlns:ns2=\"{NAMESPACE}\">\
         <ns2:FEJRESZ>\
         <ns2:KRX_VERZIOSZAM>v0.9</ns2:KRX_VERZIOSZAM>\
         <ns2:FORRASRENDSZER_AZONOSITO>KER</ns2:FORRASRENDSZER_AZONOSITO>\
         <ns2:KULDEMENY_AZONOSITO>SYNTH-0000-0000-0001</ns2:KULDEMENY_AZONOSITO>\
         <ns2:KULDEMENY_LETREHOZASANAK_IDEJE>2026-03-04T05:06:07.000+01:00\
         </ns2:KULDEMENY_LETREHOZASANAK_IDEJE>\
         <ns2:KULDEMENY_TIPUS>{kind}</ns2:KULDEMENY_TIPUS>\
         <ns2:TESZT>true</ns2:TESZT>\
         {tail}\
         </ns2:FEJRESZ>\
         "
    )
}

/// The accepted sample's shape: a header, then one attachment reference (M10).
fn accepted_sample() -> String {
    format!(
        "{}<ns2:EXPEDIALASOK><ns2:EXPEDIALAS>\
         <ns2:MELLEKLETEK_SZAMA>1</ns2:MELLEKLETEK_SZAMA>\
         <ns2:MELLEKLETEK><ns2:MELLEKLET>\
         <ns2:MELLEKLET_LEIRASA>synthetic dispatch document</ns2:MELLEKLET_LEIRASA>\
         <ns2:CSATOLMANY_SZAMA>1</ns2:CSATOLMANY_SZAMA>\
         <ns2:FAJL_NEV>synthetic-dispatch.pdf</ns2:FAJL_NEV>\
         <ns2:MERET>58.7</ns2:MERET>\
         <ns2:ELHELYEZKEDES>KRX/OCD/Payload/ID-1</ns2:ELHELYEZKEDES>\
         </ns2:MELLEKLET></ns2:MELLEKLETEK>\
         <KEZELESI_UTASITASOK>synthetic handling instruction</KEZELESI_UTASITASOK>\
         </ns2:EXPEDIALAS></ns2:EXPEDIALASOK></ns2:KULDEMENY>",
        sample("KULDEMENY", "<ns2:VONALKOD>SYNTH0000000001</ns2:VONALKOD>")
    )
}

/// The rejected sample's shape: an error report, with no attachment list.
fn rejected_sample() -> String {
    format!(
        "{}</ns2:KULDEMENY>",
        sample(
            "HIBAJELZES",
            "<ns2:HIBAKOD>SYNTH-ERR-01</ns2:HIBAKOD>\
             <ns2:KULDEMENY_MEGJEGYZES>synthetic rejection note</ns2:KULDEMENY_MEGJEGYZES>"
        )
    )
}

#[test]
fn the_accepted_sample_structure_parses_into_the_documented_shape() {
    let parsed = metadata::parse(accepted_sample().as_bytes(), &MetadataLimits::DEFAULT)
        .expect("the sample structure is a KER_META_V0_9 document");
    assert_eq!(parsed.header.version, "v0.9");
    assert_eq!(parsed.header.source_system, SourceSystem::Ker);
    assert_eq!(parsed.header.consignment_kind, ConsignmentKind::Kuldemeny);
    assert!(parsed.header.test);
    assert_eq!(parsed.header.barcode.as_deref(), Some("SYNTH0000000001"));
    assert_eq!(parsed.attachment_count(), 1);
    assert!(parsed.dispatches[0].handling_instructions_unqualified);
    // M10: the reference is the join of ELHELYEZKEDES and FAJL_NEV.
    let attachment = parsed.attachments().next().expect("one reference");
    assert_eq!(attachment.location, "KRX/OCD/Payload/ID-1");
    assert_eq!(attachment.file_name, "synthetic-dispatch.pdf");
    assert_eq!(
        attachment.declared_path(),
        "KRX/OCD/Payload/ID-1/synthetic-dispatch.pdf"
    );
}

#[test]
fn the_rejected_sample_structure_parses_into_an_error_report() {
    let parsed = metadata::parse(rejected_sample().as_bytes(), &MetadataLimits::DEFAULT)
        .expect("the sample structure is a KER_META_V0_9 document");
    assert_eq!(parsed.header.consignment_kind, ConsignmentKind::Hibajelzes);
    assert_eq!(parsed.header.error_code.as_deref(), Some("SYNTH-ERR-01"));
    assert_eq!(
        parsed.header.note.as_deref(),
        Some("synthetic rejection note")
    );
    assert_eq!(parsed.attachment_count(), 0);
    assert!(parsed.dispatches.is_empty());
}

#[test]
fn the_accepted_sample_structure_resolves_inside_the_documented_layout() {
    let image = Archive::of(vec![
        Entry::stored(b"mimetype", MARKER_CONTENT),
        Entry::deflated(
            format!("KRX/OCD/Metalayer/{METADATA_FILE}").as_bytes(),
            accepted_sample().as_bytes(),
        ),
        Entry::stored(
            b"KRX/OCD/Payload/ID-1/synthetic-dispatch.pdf",
            b"synthetic payload bytes",
        ),
    ])
    .build();
    let inventory = archive::inventory(&image, &Limits::DEFAULT).expect("archive is accepted");
    let report = profile::check(&inventory, &MetadataLimits::DEFAULT).expect("checks can run");

    for id in [
        CheckId::MetadataLocation,
        CheckId::MetadataFileName,
        CheckId::RootPrefix,
        CheckId::MarkerEntry,
        CheckId::MetadataParse,
        CheckId::SchemaOptionalFields,
        CheckId::HandlingInstructionsForm,
        CheckId::AttachmentReferences,
        CheckId::AttachmentCount,
        CheckId::AttachmentUniqueness,
    ] {
        assert_eq!(report.outcome(id), CheckOutcome::Pass, "{}", id.as_str());
    }
    // The declared size stays unresolved even here: M13 leaves the unit open.
    assert_eq!(
        report.outcome(CheckId::DeclaredSize),
        CheckOutcome::Unresolved(RuleId::M13)
    );
    assert_eq!(
        report.attachments()[0].resolution,
        ReferenceResolution::Resolved { entry: 2 }
    );
    assert_eq!(report.attachments()[0].declared_size_value, Some(58.7));
    assert_eq!(report.attachments()[0].observed_size, Some(23));
}
