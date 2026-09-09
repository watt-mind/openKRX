//! Limit boundaries, unsafe names, name collisions and unsupported features.
//!
//! Every limit is exercised immediately below and at its boundary, so a check
//! cannot be quietly loosened without a test failing.

mod support;

use openkrx_core::{Limits, archive};
use support::{Archive, Entry, assemble, split_directory};

#[track_caller]
fn rejects_with(image: &[u8], limits: &Limits, code: &str) {
    let error = archive::inventory(image, limits)
        .err()
        .unwrap_or_else(|| panic!("expected {code}, archive was accepted"));
    assert_eq!(error.code(), code);
}

#[track_caller]
fn rejects(image: &[u8], code: &str) {
    rejects_with(image, &Limits::DEFAULT, code);
}

#[track_caller]
fn accepts(image: &[u8], limits: &Limits) {
    assert!(
        archive::inventory(image, limits).is_ok(),
        "archive should have been accepted"
    );
}

// --- limit boundaries --------------------------------------------------------

#[test]
fn archive_size_limit_holds_at_its_boundary() {
    let image = Archive::of(vec![Entry::stored(b"a.txt", b"payload")]).build();
    let exact = image.len() as u64;
    let at = Limits {
        max_archive_bytes: exact,
        ..Limits::DEFAULT
    };
    let above = Limits {
        max_archive_bytes: exact + 1,
        ..Limits::DEFAULT
    };
    let below = Limits {
        max_archive_bytes: exact - 1,
        ..Limits::DEFAULT
    };
    accepts(&image, &at);
    accepts(&image, &above);
    rejects_with(&image, &below, "archive.over_limit.archive_bytes");
}

#[test]
fn entry_count_limit_holds_at_its_boundary() {
    let image = Archive::of(vec![
        Entry::stored(b"a.txt", b"one"),
        Entry::stored(b"b.txt", b"two"),
    ])
    .build();
    accepts(
        &image,
        &Limits {
            max_entries: 2,
            ..Limits::DEFAULT
        },
    );
    rejects_with(
        &image,
        &Limits {
            max_entries: 1,
            ..Limits::DEFAULT
        },
        "archive.over_limit.entries",
    );
}

#[test]
fn name_length_limit_holds_at_its_boundary() {
    let image = Archive::of(vec![Entry::stored(b"abcdefgh", b"x")]).build();
    accepts(
        &image,
        &Limits {
            max_name_bytes: 8,
            ..Limits::DEFAULT
        },
    );
    rejects_with(
        &image,
        &Limits {
            max_name_bytes: 7,
            ..Limits::DEFAULT
        },
        "archive.over_limit.name_bytes",
    );
    let long = vec![b'n'; 300];
    rejects(
        &Archive::of(vec![Entry::stored(&long, b"x")]).build(),
        "archive.over_limit.name_bytes",
    );
}

#[test]
fn per_entry_decoded_size_limit_holds_at_its_boundary() {
    let payload = support::pseudo_random(1000);
    let image = Archive::of(vec![Entry::deflated(b"a.bin", &payload)]).build();
    accepts(
        &image,
        &Limits {
            max_entry_decoded_bytes: 1000,
            ..Limits::DEFAULT
        },
    );
    rejects_with(
        &image,
        &Limits {
            max_entry_decoded_bytes: 999,
            ..Limits::DEFAULT
        },
        "archive.over_limit.entry_decoded_bytes",
    );
}

#[test]
fn total_decoded_size_limit_holds_at_its_boundary() {
    let payload = support::pseudo_random(1000);
    let image = Archive::of(vec![
        Entry::stored(b"a.bin", &payload),
        Entry::stored(b"b.bin", &payload),
    ])
    .build();
    accepts(
        &image,
        &Limits {
            max_total_decoded_bytes: 2000,
            ..Limits::DEFAULT
        },
    );
    rejects_with(
        &image,
        &Limits {
            max_total_decoded_bytes: 1999,
            ..Limits::DEFAULT
        },
        "archive.over_limit.total_decoded_bytes",
    );
}

#[test]
fn a_deflate_bomb_is_stopped_by_the_ratio_limit() {
    let payload = vec![0_u8; 4 * 1024 * 1024];
    let entry = Entry::deflated(b"bomb.bin", &payload);
    let compressed = entry.data.len() as u64;
    let image = Archive::of(vec![entry]).build();
    let ratio = payload.len() as u64 / compressed;
    assert!(ratio > Limits::DEFAULT.max_compression_ratio);
    rejects(&image, "archive.over_limit.compression_ratio");
    accepts(
        &image,
        &Limits {
            max_compression_ratio: ratio + 1,
            ..Limits::DEFAULT
        },
    );
    rejects_with(
        &image,
        &Limits {
            max_compression_ratio: ratio - 1,
            ..Limits::DEFAULT
        },
        "archive.over_limit.compression_ratio",
    );
}

#[test]
fn a_deflate_bomb_is_also_stopped_by_the_total_limit() {
    let payload = vec![0_u8; 1024 * 1024];
    let image = Archive::of(vec![
        Entry::deflated(b"one.bin", &payload),
        Entry::deflated(b"two.bin", &payload),
    ])
    .build();
    rejects_with(
        &image,
        &Limits {
            max_compression_ratio: u64::MAX,
            max_total_decoded_bytes: 1024 * 1024 + 4096,
            ..Limits::DEFAULT
        },
        "archive.over_limit.total_decoded_bytes",
    );
}

#[test]
fn a_deflate_bomb_is_also_stopped_by_the_per_entry_limit() {
    let payload = vec![0_u8; 1024 * 1024];
    let image = Archive::of(vec![Entry::deflated(b"one.bin", &payload)]).build();
    rejects_with(
        &image,
        &Limits {
            max_compression_ratio: u64::MAX,
            max_entry_decoded_bytes: 256 * 1024,
            ..Limits::DEFAULT
        },
        "archive.over_limit.entry_decoded_bytes",
    );
}

#[test]
fn a_small_entry_with_a_poor_ratio_stays_inside_the_grace_window() {
    // Exactly the grace size, compressing far better than the ratio allows:
    // the check must not fire, because the decoded volume is still bounded.
    let payload = vec![0_u8; Limits::RATIO_GRACE_BYTES as usize];
    let entry = Entry::deflated(b"zeros.bin", &payload);
    assert!(payload.len() as u64 / entry.data.len() as u64 > Limits::DEFAULT.max_compression_ratio);
    accepts(&Archive::of(vec![entry]).build(), &Limits::DEFAULT);
}

#[test]
fn extra_field_limit_holds_at_its_boundary_in_both_headers() {
    let mut central = Entry::stored(b"a.txt", b"x");
    central.central_extra = vec![0xaa; 16];
    let central = Archive::of(vec![central]).build();
    let mut local = Entry::stored(b"a.txt", b"x");
    local.local_extra = vec![0xaa; 16];
    let local = Archive::of(vec![local]).build();
    for image in [&central, &local] {
        accepts(
            image,
            &Limits {
                max_extra_field_bytes: 16,
                ..Limits::DEFAULT
            },
        );
        rejects_with(
            image,
            &Limits {
                max_extra_field_bytes: 15,
                ..Limits::DEFAULT
            },
            "archive.over_limit.extra_field_bytes",
        );
    }
}

#[test]
fn comment_limit_holds_at_its_boundary_for_archive_and_entry() {
    let mut entry = Entry::stored(b"a.txt", b"x");
    entry.central_comment = vec![b'c'; 16];
    let entry_comment = Archive::of(vec![entry]).build();
    let mut archive = Archive::of(vec![Entry::stored(b"a.txt", b"x")]);
    archive.comment = vec![b'c'; 16];
    let archive_comment = archive.build();
    for image in [&entry_comment, &archive_comment] {
        accepts(
            image,
            &Limits {
                max_comment_bytes: 16,
                ..Limits::DEFAULT
            },
        );
        rejects_with(
            image,
            &Limits {
                max_comment_bytes: 15,
                ..Limits::DEFAULT
            },
            "archive.over_limit.comment_bytes",
        );
    }
}

// --- unsafe names ------------------------------------------------------------

#[test]
fn every_unsafe_name_class_is_rejected() {
    let cases: [(&[u8], &str); 15] = [
        (b"", "archive.unsafe_name.empty"),
        (b"a\0b", "archive.unsafe_name.control_byte"),
        (b"a\x01b", "archive.unsafe_name.control_byte"),
        (b"a\nb", "archive.unsafe_name.control_byte"),
        (b"a\x7fb", "archive.unsafe_name.control_byte"),
        (b"dir\\file", "archive.unsafe_name.backslash"),
        (b"/etc/passwd", "archive.unsafe_name.absolute_path"),
        (b"../escape", "archive.unsafe_name.parent_component"),
        (
            b"Payload/../../escape",
            "archive.unsafe_name.parent_component",
        ),
        (b"Payload/..", "archive.unsafe_name.parent_component"),
        (b"c:/windows", "archive.unsafe_name.drive_prefix"),
        (b"C:", "archive.unsafe_name.drive_prefix"),
        (b"z:relative", "archive.unsafe_name.drive_prefix"),
        (b"//C:/windows", "archive.unsafe_name.absolute_path"),
        (b"Payload/", "archive.unsafe_name.directory_with_data"),
    ];
    for (name, code) in cases {
        let image = Archive::of(vec![Entry::stored(name, b"payload")]).build();
        rejects(&image, code);
    }
}

#[test]
fn names_that_merely_look_unusual_are_accepted() {
    // Nothing here is a profile judgement: A19 and A22 leave layout open, so a
    // reader may not reject a plausible-but-unexpected shape.
    for name in [
        b"..hidden".as_slice(),
        b"a..b".as_slice(),
        b"KRX/OCD/Payload/ID_1/file.pdf".as_slice(),
        b"deeply/nested/but/relative/file".as_slice(),
        b"c-drive:not-a-prefix".as_slice(),
    ] {
        accepts(
            &Archive::of(vec![Entry::stored(name, b"payload")]).build(),
            &Limits::DEFAULT,
        );
    }
}

#[test]
fn duplicate_and_case_folded_names_are_ambiguities() {
    rejects(
        &Archive::of(vec![
            Entry::stored(b"mimetype", b"one"),
            Entry::stored(b"mimetype", b"two"),
        ])
        .build(),
        "archive.ambiguous.duplicate_name",
    );
    rejects(
        &Archive::of(vec![
            Entry::stored(b"Metalayer/KULDEMENY_META.xml", b"one"),
            Entry::stored(b"metalayer/kuldemeny_meta.xml", b"two"),
        ])
        .build(),
        "archive.ambiguous.case_folded_duplicate_name",
    );
    rejects(
        &Archive::of(vec![
            Entry::stored("ÁRVÍZ.txt".as_bytes(), b"one"),
            Entry::stored("árvíz.txt".as_bytes(), b"two"),
        ])
        .build(),
        "archive.ambiguous.case_folded_duplicate_name",
    );
    rejects(
        &Archive::of(vec![
            Entry::stored(&[0xc1, b'.', b'A'], b"one"),
            Entry::stored(&[0xc1, b'.', b'a'], b"two"),
        ])
        .build(),
        "archive.ambiguous.case_folded_duplicate_name",
    );
}

// --- unsupported features ----------------------------------------------------

#[test]
fn unsupported_compression_methods_are_rejected() {
    for method in [1_u16, 6, 9, 12, 14, 93, 99] {
        let mut entry = Entry::stored(b"a.txt", b"payload");
        entry.method = method;
        rejects(
            &Archive::of(vec![entry]).build(),
            "archive.unsupported.method",
        );
    }
}

#[test]
fn encryption_and_patched_data_flags_are_rejected() {
    for (flag, code) in [
        (1_u16 << 0, "archive.unsupported.encryption"),
        (1 << 6, "archive.unsupported.encryption"),
        (1 << 13, "archive.unsupported.encryption"),
        (1 << 5, "archive.unsupported.patched_data"),
    ] {
        let mut entry = Entry::stored(b"a.txt", b"payload");
        entry.flags = flag;
        rejects(&Archive::of(vec![entry]).build(), code);
    }
}

#[test]
fn zip64_records_and_markers_are_rejected() {
    let mut marker = Entry::stored(b"a.txt", b"payload");
    marker.central_compressed = Some(u32::MAX);
    rejects(
        &Archive::of(vec![marker]).build(),
        "archive.unsupported.zip64",
    );
    let mut offset = Entry::stored(b"a.txt", b"payload");
    offset.local_offset = Some(u32::MAX);
    rejects(
        &Archive::of(vec![offset]).build(),
        "archive.unsupported.zip64",
    );
    let mut extra = Entry::stored(b"a.txt", b"payload");
    extra.central_extra = vec![0x01, 0x00, 0x00, 0x00];
    rejects(
        &Archive::of(vec![extra]).build(),
        "archive.unsupported.zip64",
    );
    let mut nested = Entry::stored(b"a.txt", b"payload");
    nested.central_extra = vec![0x99, 0x99, 0x02, 0x00, 0xaa, 0xbb, 0x01, 0x00, 0x00, 0x00];
    rejects(
        &Archive::of(vec![nested]).build(),
        "archive.unsupported.zip64",
    );
}

#[test]
fn a_truncated_extra_field_block_is_not_read_as_zip64() {
    let mut entry = Entry::stored(b"a.txt", b"payload");
    entry.central_extra = vec![0x99, 0x99, 0x40, 0x00, 0xaa];
    accepts(&Archive::of(vec![entry]).build(), &Limits::DEFAULT);
}

#[test]
fn a_record_on_another_disk_is_rejected() {
    let mut entry = Entry::stored(b"a.txt", b"payload");
    entry.disk_start = 1;
    rejects(
        &Archive::of(vec![entry]).build(),
        "archive.unsupported.multi_disk",
    );
}

#[test]
fn the_entry_count_limit_is_checked_before_the_directory_is_walked() {
    // A huge declared count must be refused from the end record alone, without
    // allocating or scanning anything proportional to it.
    let image = Archive::of(vec![Entry::stored(b"a.txt", b"payload")]).build();
    let (body, directory) = split_directory(&image);
    rejects(
        &assemble(&body, &directory, u16::MAX - 1),
        "archive.over_limit.entries",
    );
}

#[test]
fn a_ratio_exactly_at_the_limit_is_accepted_and_one_above_it_is_not() {
    // Past the grace window the ratio is compared against the limit, and the
    // comparison is strict: a ratio equal to the limit is inside it. The
    // limit is set from what this payload actually achieves, so the boundary
    // is the measured value rather than a guess about the deflate stream.
    let payload = vec![0_u8; 4 * Limits::RATIO_GRACE_BYTES as usize];
    let entry = Entry::deflated(b"zeros.bin", &payload);
    let ratio = payload.len() as u64 / entry.data.len() as u64;
    assert!(ratio > 1, "the payload compresses at all");
    let image = Archive::of(vec![entry]).build();

    accepts(
        &image,
        &Limits {
            max_compression_ratio: ratio,
            max_entry_decoded_bytes: payload.len() as u64,
            max_total_decoded_bytes: payload.len() as u64,
            ..Limits::DEFAULT
        },
    );
    rejects_with(
        &image,
        &Limits {
            max_compression_ratio: ratio - 1,
            max_entry_decoded_bytes: payload.len() as u64,
            max_total_decoded_bytes: payload.len() as u64,
            ..Limits::DEFAULT
        },
        "archive.over_limit.compression_ratio",
    );
}

#[test]
fn an_end_record_declaring_the_zip64_entry_sentinel_is_refused_as_zip64() {
    // 0xFFFF in the end record's entry count is the sentinel that says the
    // real count lives in a ZIP64 end record. It is not an entry count of
    // 65 535, and it must not be read as one: the archive is refused as an
    // unsupported ZIP64 image rather than as one over the entry limit.
    // Sibling sentinels for the directory size and offset are the same rule,
    // but reaching them needs a 4 GiB image; see docs/testing.md.
    let image = Archive::of(vec![Entry::stored(b"a.txt", b"payload")]).build();
    let (body, directory) = split_directory(&image);
    rejects(
        &assemble(&body, &directory, u16::MAX),
        "archive.unsupported.zip64",
    );
}

#[test]
fn a_directory_name_carrying_a_compressed_payload_is_refused() {
    // A name ending in `/` may only be a directory marker with nothing in it.
    // "Nothing in it" is either declared size being zero — and a deflate
    // stream of no bytes is still two bytes long, so an entry can declare a
    // compressed size while declaring no content at all. Both readings must
    // refuse it, or a directory marker could smuggle a payload.
    let image = Archive::of(vec![Entry::deflated(b"dir/", b"")]).build();
    rejects(&image, "archive.unsafe_name.directory_with_data");
}
