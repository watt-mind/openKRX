//! Accepted-archive behaviour: what the inventory reports and what it refuses
//! to conclude.

mod support;

use openkrx_core::{ArchiveError, Limits, archive};
use support::{Archive, DEFLATE, Entry, FLAG_UTF8, STORED, crc32, marker_archive};

fn read(image: &[u8]) -> archive::ArchiveInventory<'_> {
    archive::inventory(image, &Limits::DEFAULT).expect("archive should be accepted")
}

#[test]
fn stored_and_deflated_entries_are_listed_in_central_directory_order() {
    let image = marker_archive();
    let inventory = read(&image);
    assert_eq!(inventory.len(), 2);
    assert!(!inventory.is_empty());
    let names: Vec<&[u8]> = inventory
        .entries()
        .iter()
        .map(openkrx_core::archive::ArchiveEntry::name_bytes)
        .collect();
    assert_eq!(
        names,
        vec![
            b"mimetype".as_slice(),
            b"Metalayer/KULDEMENY_META.xml".as_slice()
        ]
    );
    assert_eq!(inventory.entries()[0].method(), STORED);
    assert_eq!(inventory.entries()[1].method(), DEFLATE);
}

#[test]
fn entry_reports_declared_metadata_and_counted_decoded_size() {
    let payload = b"application/OCD+ZIP";
    let image = Archive::of(vec![Entry::deflated(b"mimetype", payload)]).build();
    let inventory = read(&image);
    let entry = inventory.entries()[0];
    assert_eq!(entry.name_text(), Some("mimetype"));
    assert!(!entry.utf8_flag());
    assert_eq!(entry.flags(), 0);
    assert_eq!(entry.crc32(), crc32(payload));
    assert_eq!(entry.uncompressed_size(), payload.len() as u64);
    assert_eq!(entry.decoded_size(), payload.len() as u64);
    assert!(entry.compressed_size() > 0);
    assert_eq!(entry.local_header_offset(), 0);
    assert_eq!(inventory.total_decoded_bytes(), payload.len() as u64);
}

#[test]
fn utf8_flag_and_decodable_name_are_reported_independently() {
    let mut flagged = Entry::stored("árvíz".as_bytes(), b"a");
    flagged.flags = FLAG_UTF8;
    let latin1 = Entry::stored(&[0xe1, 0x72, 0x76], b"b");
    let image = Archive::of(vec![flagged, latin1]).build();
    let inventory = read(&image);
    assert!(inventory.entries()[0].utf8_flag());
    assert_eq!(inventory.entries()[0].name_text(), Some("árvíz"));
    assert!(!inventory.entries()[1].utf8_flag());
    assert_eq!(inventory.entries()[1].name_text(), None);
    assert_eq!(inventory.entries()[1].name_bytes(), &[0xe1, 0x72, 0x76]);
}

#[test]
fn observation_helpers_never_assert_conformance() {
    let inventory_bytes = marker_archive();
    let inventory = read(&inventory_bytes);
    assert!(inventory.is_first_entry_named(b"mimetype"));
    assert!(!inventory.is_first_entry_named(b"KRX/mimetype"));
    assert!(
        inventory
            .find_by_name(b"Metalayer/KULDEMENY_META.xml")
            .is_some()
    );
    assert!(inventory.find_by_name(b"missing").is_none());

    // The same helper answers "no" for the equally plausible nested layout,
    // because profile rule A19 leaves the root prefix unresolved.
    let nested = Archive::of(vec![Entry::stored(b"KRX/mimetype", b"application/OCD+ZIP")]).build();
    let nested = read(&nested);
    assert!(!nested.is_first_entry_named(b"mimetype"));
    assert!(nested.is_first_entry_named(b"KRX/mimetype"));
}

#[test]
fn empty_archive_is_accepted_and_reports_nothing() {
    let image = Archive::default().build();
    let inventory = read(&image);
    assert!(inventory.is_empty());
    assert_eq!(inventory.total_decoded_bytes(), 0);
    assert!(!inventory.is_first_entry_named(b"mimetype"));
}

#[test]
fn zero_length_and_directory_entries_are_accepted() {
    let image = Archive::of(vec![
        Entry::stored(b"Payload/", b""),
        Entry::stored(b"Payload/ID-1/empty.bin", b""),
    ])
    .build();
    let inventory = read(&image);
    assert_eq!(inventory.len(), 2);
    assert_eq!(inventory.entries()[0].decoded_size(), 0);
    assert_eq!(inventory.entry_bytes(1).expect("empty entry"), Vec::new());
}

#[test]
fn entry_bytes_redecodes_only_the_requested_entry() {
    let image = marker_archive();
    let inventory = read(&image);
    assert_eq!(
        inventory.entry_bytes(0).expect("marker"),
        b"application/OCD+ZIP"
    );
    assert_eq!(inventory.entry_bytes(1).expect("metadata"), b"<KULDEMENY/>");
    assert_eq!(
        inventory.entry_bytes(2).unwrap_err().code(),
        "archive.no_such_entry"
    );
    assert_eq!(
        inventory.entry_bytes(u32::MAX).unwrap_err().code(),
        "archive.no_such_entry"
    );
}

#[test]
fn entry_bytes_reproduces_a_large_deflated_payload_exactly() {
    let payload = support::pseudo_random(200_000);
    let image = Archive::of(vec![Entry::deflated(b"Payload/ID-1/blob.bin", &payload)]).build();
    let inventory = read(&image);
    assert_eq!(inventory.entries()[0].decoded_size(), payload.len() as u64);
    assert_eq!(inventory.entry_bytes(0).expect("payload"), payload);
}

#[test]
fn data_descriptors_are_accepted_with_and_without_their_signature() {
    for signed in [true, false] {
        let image = Archive::of(vec![
            Entry::deflated(b"a.txt", b"first entry contents").with_descriptor(signed),
            Entry::stored(b"b.txt", b"second").with_descriptor(signed),
        ])
        .build();
        let inventory = read(&image);
        assert_eq!(inventory.len(), 2);
        assert_eq!(inventory.entry_bytes(1).expect("second"), b"second");
    }
}

#[test]
fn archive_comment_extra_fields_and_entry_comments_are_tolerated() {
    let mut entry = Entry::stored(b"mimetype", b"application/OCD+ZIP");
    entry.local_extra = vec![0x99, 0x99, 2, 0, 1, 2];
    entry.central_extra = vec![0x98, 0x98, 1, 0, 7];
    entry.central_comment = b"entry comment".to_vec();
    let mut archive = Archive::of(vec![entry]);
    archive.comment = b"archive comment".to_vec();
    let image = archive.build();
    let inventory = read(&image);
    assert!(inventory.is_first_entry_named(b"mimetype"));
}

#[test]
fn limits_are_the_documented_defaults() {
    let limits = Limits::DEFAULT;
    assert_eq!(limits, Limits::default());
    assert_eq!(limits.max_archive_bytes, 64 * 1024 * 1024);
    assert_eq!(limits.max_entries, 256);
    assert_eq!(limits.max_name_bytes, 255);
    assert_eq!(limits.max_entry_decoded_bytes, 32 * 1024 * 1024);
    assert_eq!(limits.max_total_decoded_bytes, 128 * 1024 * 1024);
    assert_eq!(limits.max_compression_ratio, 100);
    assert_eq!(limits.max_extra_field_bytes, 4 * 1024);
    assert_eq!(limits.max_comment_bytes, 1024);
    assert_eq!(Limits::RATIO_GRACE_BYTES, 64 * 1024);
    assert_eq!(Limits::OUTPUT_BUFFER_BYTES, 64 * 1024);
}

#[test]
fn error_display_carries_codes_and_numbers_but_no_entry_name() {
    let image = Archive::of(vec![Entry::stored("titkos-ügyirat.pdf".as_bytes(), b"x")]).build();
    let mut limits = Limits::DEFAULT;
    limits.max_name_bytes = 4;
    let error = archive::inventory(&image, &limits).unwrap_err();
    let rendered = error.to_string();
    assert_eq!(error.code(), "archive.over_limit.name_bytes");
    assert_eq!(error.entry_index(), Some(0));
    assert!(rendered.starts_with("archive.over_limit.name_bytes"));
    assert!(rendered.contains("limit 4"));
    assert!(rendered.contains("at entry 0"));
    assert!(!rendered.contains("ügyirat"));
    assert!(!rendered.contains("pdf"));
    let _: &dyn std::error::Error = &error;
}

#[test]
fn unsupported_display_reports_the_offending_value_only() {
    let mut entry = Entry::stored(b"a.txt", b"x");
    entry.method = 12;
    entry.central_method = Some(12);
    let image = Archive::of(vec![entry]).build();
    let error = archive::inventory(&image, &Limits::DEFAULT).unwrap_err();
    assert_eq!(error.code(), "archive.unsupported.method");
    assert!(error.to_string().contains("value 12"));
}

#[test]
fn archive_scoped_errors_carry_no_entry_index() {
    let error: ArchiveError = archive::inventory(b"", &Limits::DEFAULT).unwrap_err();
    assert_eq!(error.entry_index(), None);
    assert_eq!(error.to_string(), "archive.malformed.eocd_missing");
}
