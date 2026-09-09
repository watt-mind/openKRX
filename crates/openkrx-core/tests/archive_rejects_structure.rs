//! Structural rejection: contradictory records, ambiguity, coverage gaps,
//! truncation and corrupt streams.
//!
//! Each test asserts the stable error code, so a category can never silently
//! turn into another one.

mod support;

use openkrx_core::{Limits, archive};
use support::{
    Archive, CENTRAL_SIGNATURE, DESCRIPTOR_SIGNATURE, EOCD_SIGNATURE, Entry, LOCAL_SIGNATURE,
    assemble, marker_archive, split_directory,
};

#[track_caller]
fn rejects(image: &[u8], code: &str) {
    let error = archive::inventory(image, &Limits::DEFAULT)
        .err()
        .unwrap_or_else(|| panic!("expected {code}, archive was accepted"));
    assert_eq!(error.code(), code);
}

// --- end-of-central-directory ------------------------------------------------

#[test]
fn missing_end_record_is_reported_as_missing() {
    rejects(b"", "archive.malformed.eocd_missing");
    rejects(
        b"PK\x03\x04 not really an archive",
        "archive.malformed.eocd_missing",
    );
    rejects(&[0_u8; 512], "archive.malformed.eocd_missing");
}

#[test]
fn bytes_after_the_end_record_comment_are_rejected() {
    let mut archive = Archive::of(vec![Entry::stored(b"a.txt", b"a")]);
    archive.suffix = b"appended".to_vec();
    rejects(&archive.build(), "archive.malformed.trailing_bytes");
}

#[test]
fn a_second_end_record_candidate_is_an_ambiguity_not_a_choice() {
    // The archive comment is itself a complete end record whose own comment
    // reaches the end of the image, so two readings terminate the file.
    let mut inner = Vec::new();
    inner.extend_from_slice(&EOCD_SIGNATURE);
    inner.extend_from_slice(&[0; 18]);
    let mut archive = Archive::of(vec![Entry::stored(b"a.txt", b"a")]);
    archive.comment = inner;
    rejects(&archive.build(), "archive.ambiguous.eocd");
}

#[test]
fn multi_disk_end_records_are_unsupported() {
    for archive in [
        Archive {
            eocd_disk: Some(1),
            ..Archive::of(vec![Entry::stored(b"a.txt", b"a")])
        },
        Archive {
            eocd_directory_disk: Some(1),
            ..Archive::of(vec![Entry::stored(b"a.txt", b"a")])
        },
        Archive {
            eocd_entries_here: Some(0),
            ..Archive::of(vec![Entry::stored(b"a.txt", b"a")])
        },
    ] {
        rejects(&archive.build(), "archive.unsupported.multi_disk");
    }
}

#[test]
fn a_zip64_locator_before_the_end_record_is_unsupported() {
    let image = Archive::of(vec![Entry::stored(b"a.txt", b"a")]).build();
    let (body, mut directory) = split_directory(&image);
    directory.extend_from_slice(&[b'P', b'K', 6, 7]);
    rejects(&assemble(&body, &directory, 1), "archive.unsupported.zip64");
}

#[test]
fn zip64_marker_values_in_the_end_record_are_unsupported() {
    let entry = Entry::stored(b"a.txt", b"a");
    let archive = Archive {
        eocd_directory_offset: Some(u32::MAX),
        ..Archive::of(vec![entry])
    };
    rejects(&archive.build(), "archive.unsupported.zip64");
}

#[test]
fn a_displaced_central_directory_is_rejected() {
    for archive in [
        Archive {
            eocd_directory_offset: Some(0),
            ..Archive::of(vec![Entry::stored(b"a.txt", b"a")])
        },
        Archive {
            eocd_directory_bytes: Some(4),
            ..Archive::of(vec![Entry::stored(b"a.txt", b"a")])
        },
    ] {
        rejects(
            &archive.build(),
            "archive.malformed.central_directory_placement",
        );
    }
}

// --- central directory -------------------------------------------------------

#[test]
fn a_declared_record_count_above_the_real_one_is_rejected() {
    let archive = Archive {
        eocd_entries: Some(2),
        ..Archive::of(vec![Entry::stored(b"a.txt", b"a")])
    };
    rejects(
        &archive.build(),
        "archive.malformed.central_directory_count",
    );
}

#[test]
fn a_declared_record_count_below_the_real_one_is_rejected() {
    let archive = Archive {
        eocd_entries: Some(0),
        ..Archive::of(vec![Entry::stored(b"a.txt", b"a")])
    };
    rejects(&archive.build(), "archive.malformed.central_directory_size");
}

#[test]
fn a_record_without_its_signature_is_rejected() {
    let mut entry = Entry::stored(b"a.txt", b"a");
    entry.central_signature = Some([b'P', b'K', 9, 9]);
    rejects(
        &Archive::of(vec![entry]).build(),
        "archive.malformed.record_signature",
    );
    let mut entry = Entry::stored(b"a.txt", b"a");
    entry.local_signature = Some([b'P', b'K', 9, 9]);
    rejects(
        &Archive::of(vec![entry]).build(),
        "archive.malformed.record_signature",
    );
}

#[test]
fn a_central_record_cut_short_is_reported_as_truncation() {
    let image = Archive::of(vec![Entry::stored(b"a.txt", b"a")]).build();
    let (body, directory) = split_directory(&image);
    for kept in [4, 20, 44, directory.len() - 1] {
        rejects(
            &assemble(&body, &directory[..kept], 1),
            "archive.truncated.central_directory",
        );
    }
}

// --- local headers -----------------------------------------------------------

#[test]
fn a_local_header_contradicting_its_record_is_rejected() {
    type Contradiction = Box<dyn Fn(&mut Entry)>;
    let contradictions: Vec<Contradiction> = vec![
        Box::new(|entry| entry.local_name = Some(b"b.txt".to_vec())),
        Box::new(|entry| entry.local_flags = Some(1 << 1)),
        Box::new(|entry| entry.local_method = Some(8)),
        Box::new(|entry| entry.local_crc = Some(0xdead_beef)),
        Box::new(|entry| entry.local_uncompressed = Some(99)),
    ];
    for apply in contradictions {
        let mut entry = Entry::stored(b"a.txt", b"abcde");
        apply(&mut entry);
        rejects(
            &Archive::of(vec![entry]).build(),
            "archive.malformed.local_header_mismatch",
        );
    }
}

#[test]
fn a_local_compressed_size_below_the_record_is_rejected() {
    let mut entry = Entry::deflated(b"a.txt", b"abcdefghij");
    entry.local_compressed = Some(2);
    rejects(
        &Archive::of(vec![entry]).build(),
        "archive.malformed.local_header_mismatch",
    );
}

#[test]
fn a_record_pointing_past_the_image_is_reported_as_truncation() {
    let mut entry = Entry::stored(b"a.txt", b"a");
    entry.local_offset = Some(0xffff_fff0);
    rejects(
        &Archive::of(vec![entry]).build(),
        "archive.truncated.local_header",
    );
}

#[test]
fn an_entry_declaring_more_data_than_the_image_holds_is_truncated() {
    let mut entry = Entry::deflated(b"a.txt", b"abcdefghij");
    entry.central_compressed = Some(0x0fff_ffff);
    entry.local_compressed = Some(0x0fff_ffff);
    rejects(
        &Archive::of(vec![entry]).build(),
        "archive.truncated.entry_data",
    );
}

// --- data descriptors --------------------------------------------------------

#[test]
fn a_data_descriptor_contradicting_its_record_is_rejected() {
    for corrupt in [0_usize, 4, 8] {
        let mut entry = Entry::deflated(b"a.txt", b"descriptor payload").with_descriptor(true);
        entry.trailer[corrupt + 4] ^= 0xff;
        rejects(
            &Archive::of(vec![entry]).build(),
            "archive.malformed.data_descriptor_mismatch",
        );
    }
}

#[test]
fn an_unsigned_data_descriptor_is_still_checked() {
    let mut entry = Entry::stored(b"a.txt", b"descriptor payload").with_descriptor(false);
    entry.trailer[0] ^= 0xff;
    rejects(
        &Archive::of(vec![entry]).build(),
        "archive.malformed.data_descriptor_mismatch",
    );
}

#[test]
fn a_local_header_with_stale_values_under_bit_three_is_rejected() {
    let mut entry = Entry::deflated(b"a.txt", b"descriptor payload").with_descriptor(true);
    entry.local_uncompressed = Some(7);
    rejects(
        &Archive::of(vec![entry]).build(),
        "archive.malformed.local_header_mismatch",
    );
}

#[test]
fn a_descriptor_running_into_the_end_of_the_image_is_truncated() {
    // Stretch the declared compressed size so the data ends six bytes before
    // the end of the image, leaving no room for a complete descriptor.
    let payload = b"descriptor payload";
    let template = Entry::deflated(b"a.txt", payload).with_descriptor(true);
    let image = Archive::of(vec![template.clone()]).build();
    let data_start = 30 + template.name.len();
    let stretched = u32::try_from(image.len() - 6 - data_start).expect("fits");
    let mut entry = template;
    entry.central_compressed = Some(stretched);
    let image = Archive::of(vec![entry]).build();
    rejects(&image, "archive.truncated.data_descriptor");
}

// --- image coverage ----------------------------------------------------------

#[test]
fn bytes_before_the_first_local_header_are_rejected() {
    let mut archive = Archive::of(vec![Entry::stored(b"a.txt", b"a")]);
    archive.prefix = b"self-extracting stub".to_vec();
    rejects(&archive.build(), "archive.malformed.prefix_bytes");
}

#[test]
fn bytes_claimed_by_no_structure_are_rejected() {
    let payload = b"ten bytes!";
    let mut entry = Entry::deflated(b"a.txt", payload);
    entry.central_compressed = Some(1);
    entry.local_compressed = Some(1);
    rejects(
        &Archive::of(vec![entry]).build(),
        "archive.malformed.unclaimed_bytes",
    );
}

#[test]
fn overlapping_entry_ranges_are_rejected() {
    let mut first = Entry::deflated(b"a.txt", b"first payload");
    first.central_compressed = Some(120);
    first.local_compressed = Some(120);
    let image = Archive::of(vec![first, Entry::stored(b"b.txt", b"second payload")]).build();
    rejects(&image, "archive.malformed.overlapping_ranges");
}

#[test]
fn an_entry_range_reaching_into_the_central_directory_is_rejected() {
    let payload = b"payload bytes";
    let template = Entry::deflated(b"a.txt", payload);
    let image = Archive::of(vec![template.clone()]).build();
    let data_start = 30 + template.name.len();
    let (body, _) = split_directory(&image);
    let stretched = u32::try_from(body.len() + 8 - data_start).expect("fits");
    let mut entry = template;
    entry.central_compressed = Some(stretched);
    entry.local_compressed = Some(stretched);
    rejects(
        &Archive::of(vec![entry]).build(),
        "archive.malformed.overlapping_ranges",
    );
}

// --- entry data --------------------------------------------------------------

#[test]
fn a_crc_that_does_not_match_the_decoded_bytes_is_rejected() {
    for mut entry in [
        Entry::stored(b"a.txt", b"corruption check"),
        Entry::deflated(b"a.txt", b"corruption check"),
    ] {
        entry.crc ^= 0xffff_ffff;
        rejects(
            &Archive::of(vec![entry]).build(),
            "archive.malformed.crc_mismatch",
        );
    }
}

#[test]
fn a_declared_size_below_the_decoded_size_is_rejected() {
    let mut entry = Entry::deflated(b"a.txt", &support::pseudo_random(4096));
    entry.central_uncompressed = Some(16);
    entry.local_uncompressed = Some(16);
    rejects(
        &Archive::of(vec![entry]).build(),
        "archive.malformed.declared_size_mismatch",
    );
}

#[test]
fn a_declared_size_above_the_decoded_size_is_rejected() {
    let mut entry = Entry::deflated(b"a.txt", b"short");
    entry.central_uncompressed = Some(4096);
    entry.local_uncompressed = Some(4096);
    rejects(
        &Archive::of(vec![entry]).build(),
        "archive.malformed.declared_size_mismatch",
    );
}

#[test]
fn a_stored_entry_whose_sizes_disagree_is_rejected() {
    let mut entry = Entry::stored(b"a.txt", b"stored payload");
    entry.central_uncompressed = Some(3);
    entry.local_uncompressed = Some(3);
    rejects(
        &Archive::of(vec![entry]).build(),
        "archive.malformed.declared_size_mismatch",
    );
}

#[test]
fn a_corrupt_deflate_stream_is_rejected() {
    let payload = support::pseudo_random(4096);
    let mut entry = Entry::deflated(b"a.txt", &payload);
    entry.data[0] ^= 0xff;
    rejects(
        &Archive::of(vec![entry]).build(),
        "archive.malformed.deflate_stream",
    );
}

#[test]
fn a_deflate_stream_that_ends_early_is_rejected() {
    let payload = support::pseudo_random(4096);
    let template = Entry::deflated(b"a.txt", &payload);
    let mut entry = template.clone();
    entry.data.truncate(template.data.len() / 2);
    rejects(
        &Archive::of(vec![entry]).build(),
        "archive.malformed.deflate_stream",
    );
}

#[test]
fn trailing_bytes_inside_a_deflate_stream_are_rejected() {
    let mut entry = Entry::deflated(b"a.txt", b"payload");
    entry.data.extend_from_slice(b"extra");
    rejects(
        &Archive::of(vec![entry]).build(),
        "archive.malformed.deflate_stream",
    );
}

// --- robustness --------------------------------------------------------------

#[track_caller]
fn every_truncation_is_rejected(image: &[u8]) {
    for end in 0..image.len() {
        assert!(
            archive::inventory(&image[..end], &Limits::DEFAULT).is_err(),
            "truncation to {end} bytes was accepted"
        );
    }
}

#[test]
fn no_truncation_of_a_valid_archive_is_ever_accepted() {
    every_truncation_is_rejected(&marker_archive());
    every_truncation_is_rejected(&Archive::default().build());
    every_truncation_is_rejected(
        &Archive::of(vec![
            Entry::deflated(b"a.txt", b"payload one").with_descriptor(true),
            Entry::stored(b"Payload/ID-1/b.bin", b"payload two"),
        ])
        .build(),
    );
    let mut with_comment = Archive::of(vec![Entry::stored(b"mimetype", b"application/OCD+ZIP")]);
    with_comment.comment = b"comment".to_vec();
    every_truncation_is_rejected(&with_comment.build());
}

#[test]
fn single_byte_mutations_never_panic() {
    let image = marker_archive();
    for position in 0..image.len() {
        for replacement in [0x00_u8, 0xff, 0x50] {
            let mut mutated = image.clone();
            mutated[position] = replacement;
            let _ = archive::inventory(&mutated, &Limits::DEFAULT);
        }
    }
}

#[test]
fn stray_signatures_in_entry_data_never_panic() {
    for signature in [
        LOCAL_SIGNATURE,
        CENTRAL_SIGNATURE,
        EOCD_SIGNATURE,
        DESCRIPTOR_SIGNATURE,
    ] {
        let mut payload = signature.to_vec();
        payload.extend_from_slice(&[0; 32]);
        let image = Archive::of(vec![Entry::stored(b"a.bin", &payload)]).build();
        let _ = archive::inventory(&image, &Limits::DEFAULT);
    }
}
