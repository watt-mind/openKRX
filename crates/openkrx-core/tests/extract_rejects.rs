//! Everything the extraction planner refuses, each by its stable code.
//!
//! A planning failure rejects the whole plan, so every test here asserts that
//! `plan` produced no plan at all, not that one entry was left out.

mod support;

use openkrx_core::extract::{self, ExtractLimits};
use openkrx_core::{Limits, archive};
use support::{Archive, Entry};

/// Unix `st_mode` for a symbolic link.
const MODE_SYMLINK: u32 = 0o120_777;
/// Unix `st_mode` for a character device.
const MODE_CHARACTER_DEVICE: u32 = 0o020_666;
/// Host system 19: OS X (Darwin), which carries `st_mode` as host 3 does.
const HOST_DARWIN: u16 = 19;

#[track_caller]
fn rejects_with(image: &[u8], limits: &ExtractLimits, code: &str) -> u32 {
    let inventory = archive::inventory(image, &Limits::DEFAULT).expect("archive is readable");
    let error = extract::plan(&inventory, limits)
        .err()
        .unwrap_or_else(|| panic!("expected {code}, the plan was produced"));
    assert_eq!(error.code(), code);
    error
        .entry_index()
        .expect("a planning error names an entry")
}

#[track_caller]
fn rejects(image: &[u8], code: &str) -> u32 {
    rejects_with(image, &ExtractLimits::DEFAULT, code)
}

#[track_caller]
fn accepts(image: &[u8], limits: &ExtractLimits) {
    let inventory = archive::inventory(image, &Limits::DEFAULT).expect("archive is readable");
    assert!(
        extract::plan(&inventory, limits).is_ok(),
        "the plan should have been produced"
    );
}

/// A one-entry archive whose single file is named `name`.
fn one_file(name: &[u8]) -> Vec<u8> {
    Archive::of(vec![Entry::stored(name, b"attachment")]).build()
}

// --- unsupported entries -----------------------------------------------------

#[test]
fn a_symlink_entry_rejects_the_whole_plan() {
    let image = Archive::of(vec![
        Entry::stored(b"mimetype", b"application/OCD+ZIP"),
        Entry::stored(b"Payload/link", b"../../etc/passwd").with_unix_mode(MODE_SYMLINK),
    ])
    .build();

    assert_eq!(rejects(&image, "extract.unsupported.link"), 1);
}

#[test]
fn a_darwin_host_symlink_is_refused_like_a_unix_one() {
    // Host 19 stores `st_mode` in the high attribute bits exactly as host 3
    // does, so a link written on macOS must be read as a link rather than as
    // an unmapped host planned as an ordinary file holding its target text.
    let image = Archive::of(vec![
        Entry::stored(b"mimetype", b"application/OCD+ZIP"),
        Entry::stored(b"Payload/link", b"../../etc/passwd")
            .with_unix_mode(MODE_SYMLINK)
            .with_host_system(HOST_DARWIN),
    ])
    .build();

    assert_eq!(rejects(&image, "extract.unsupported.link"), 1);
}

#[test]
fn a_special_file_entry_rejects_the_whole_plan() {
    let device = Archive::of(vec![
        Entry::stored(b"dev", b"").with_unix_mode(MODE_CHARACTER_DEVICE),
    ])
    .build();
    assert_eq!(rejects(&device, "extract.unsupported.special_file"), 0);

    let unstated = Archive::of(vec![Entry::stored(b"dev", b"").with_unix_mode(0)]).build();
    assert_eq!(
        rejects(&unstated, "extract.unsupported.special_file"),
        0,
        "a Unix mode that states no file type is refused rather than assumed"
    );
}

#[test]
fn a_name_that_is_not_utf8_is_refused_rather_than_decoded() {
    let image = one_file(b"Payload/\xff\xfe.pdf");

    assert_eq!(rejects(&image, "extract.unsupported.non_utf8_name"), 0);
}

// --- unsafe destination components -------------------------------------------

#[test]
fn every_unsafe_component_class_reachable_from_an_archive_is_refused() {
    for (name, code) in [
        (&b"a//b"[..], "extract.unsafe_path.empty_component"),
        (&b"a/./b"[..], "extract.unsafe_path.current_component"),
        (
            &b"a/\xc2\x85/b"[..],
            "extract.unsafe_path.control_character",
        ),
        (&b"a/b."[..], "extract.unsafe_path.trailing_dot"),
        (&b"a/b "[..], "extract.unsafe_path.trailing_space"),
        (&b"a/CON"[..], "extract.unsafe_path.reserved_device_name"),
        (
            &b"a/lpt9.txt"[..],
            "extract.unsafe_path.reserved_device_name",
        ),
        (&b"a/b:c"[..], "extract.unsafe_path.colon"),
        (&b"a/ b"[..], "extract.unsafe_path.leading_space"),
        (&b"a/b*c"[..], "extract.unsafe_path.reserved_character"),
    ] {
        assert_eq!(rejects(&one_file(name), code), 0, "{code}");
    }
}

#[test]
fn every_character_windows_refuses_outright_is_one_reserved_character_class() {
    for character in *b"*?<>|\"" {
        let name = [b"Payload/x".as_slice(), &[character], b".pdf"].concat();
        assert_eq!(
            rejects(&one_file(&name), "extract.unsafe_path.reserved_character"),
            0,
            "{}",
            char::from(character)
        );
    }
    // `\` is refused by the archive layer as `archive.unsafe_name.backslash`
    // before a plan is attempted, so the planner never sees one.
    let image = one_file(b"Payload/x\\y.pdf");
    let error = archive::inventory(&image, &Limits::DEFAULT).expect_err("a backslash is refused");
    assert_eq!(error.code(), "archive.unsafe_name.backslash");
}

#[test]
fn a_trailing_separator_on_an_empty_entry_is_a_directory_not_an_empty_component() {
    let image = Archive::of(vec![
        Entry::stored(b"Payload/", b""),
        Entry::stored(b"Payload/x", b"attachment"),
    ])
    .build();

    accepts(&image, &ExtractLimits::DEFAULT);
}

// --- ambiguity ---------------------------------------------------------------

#[test]
fn two_names_equal_after_normalisation_are_a_collision_not_a_choice() {
    let image = Archive::of(vec![
        Entry::stored("Payload/\u{c1}.pdf".as_bytes(), b"one"),
        Entry::stored("Payload/A\u{301}.pdf".as_bytes(), b"two"),
    ])
    .build();

    assert_eq!(rejects(&image, "extract.ambiguous.collision"), 1);
}

#[test]
fn a_case_difference_that_only_appears_after_normalisation_is_a_collision() {
    // A byte-identical or plainly case-folded duplicate never reaches the
    // planner: the archive layer already refuses it. Only a pair the archive
    // layer's byte-wise fold keeps apart can be tested here, and folding to NFC
    // first is exactly what closes that gap.
    let image = Archive::of(vec![
        Entry::stored("Payload/\u{c1}.pdf".as_bytes(), b"one"),
        Entry::stored("Payload/a\u{301}.pdf".as_bytes(), b"two"),
    ])
    .build();

    assert_eq!(rejects(&image, "extract.ambiguous.collision"), 1);
}

#[test]
fn a_file_that_is_also_a_directory_prefix_is_refused() {
    let image = Archive::of(vec![
        Entry::stored(b"Payload", b"a file called Payload"),
        Entry::stored(b"Payload/x.pdf", b"attachment"),
    ])
    .build();

    assert_eq!(
        rejects(&image, "extract.ambiguous.file_directory_conflict"),
        1
    );
}

#[test]
fn a_directory_prefix_that_arrives_before_its_file_is_refused_the_same_way() {
    // The mirror of the test above: the deeper name comes first, so the
    // conflict is only visible once the shallow one is planned. The reported
    // entry is the later of the two either way.
    let image = Archive::of(vec![
        Entry::stored(b"Payload/x.pdf", b"attachment"),
        Entry::stored(b"Payload", b"a file called Payload"),
    ])
    .build();

    assert_eq!(
        rejects(&image, "extract.ambiguous.file_directory_conflict"),
        1
    );
}

#[test]
fn a_shared_file_name_in_different_directories_is_not_a_collision() {
    let image = Archive::of(vec![
        Entry::stored(b"Payload/ID-1/x.pdf", b"one"),
        Entry::stored(b"Payload/ID-2/x.pdf", b"two"),
    ])
    .build();

    accepts(&image, &ExtractLimits::DEFAULT);
}

// --- limit boundaries --------------------------------------------------------

#[test]
fn the_file_count_limit_holds_at_its_boundary() {
    let image = Archive::of(vec![
        Entry::stored(b"a.txt", b"one"),
        Entry::stored(b"b.txt", b"two"),
        Entry::stored(b"dir/", b""),
    ])
    .build();

    accepts(
        &image,
        &ExtractLimits {
            max_files: 3,
            ..ExtractLimits::DEFAULT
        },
    );
    accepts(
        &image,
        &ExtractLimits {
            max_files: 2,
            ..ExtractLimits::DEFAULT
        },
    );
    assert_eq!(
        rejects_with(
            &image,
            &ExtractLimits {
                max_files: 1,
                ..ExtractLimits::DEFAULT
            },
            "extract.over_limit.files",
        ),
        1,
        "a directory marker is not a file and does not count"
    );
}

#[test]
fn the_total_size_limit_holds_at_its_boundary() {
    let image = Archive::of(vec![
        Entry::stored(b"a.txt", b"one"),
        Entry::stored(b"b.txt", b"four"),
    ])
    .build();
    let exact = 7;

    accepts(
        &image,
        &ExtractLimits {
            max_total_bytes: exact,
            ..ExtractLimits::DEFAULT
        },
    );
    assert_eq!(
        rejects_with(
            &image,
            &ExtractLimits {
                max_total_bytes: exact - 1,
                ..ExtractLimits::DEFAULT
            },
            "extract.over_limit.total_bytes",
        ),
        1
    );
    assert_eq!(
        rejects_with(
            &image,
            &ExtractLimits {
                max_total_bytes: 0,
                ..ExtractLimits::DEFAULT
            },
            "extract.over_limit.total_bytes",
        ),
        0
    );
}

#[test]
fn the_path_length_limit_holds_at_its_boundary() {
    let image = one_file(b"Payload/x.pdf");
    let exact = u32::try_from("Payload/x.pdf".len()).expect("a test name fits in u32");

    accepts(
        &image,
        &ExtractLimits {
            max_path_bytes: exact,
            ..ExtractLimits::DEFAULT
        },
    );
    assert_eq!(
        rejects_with(
            &image,
            &ExtractLimits {
                max_path_bytes: exact - 1,
                ..ExtractLimits::DEFAULT
            },
            "extract.over_limit.path_bytes",
        ),
        0
    );
}

#[test]
fn the_component_length_limit_holds_at_its_boundary() {
    let image = one_file(b"Payload/x.pdf");

    accepts(
        &image,
        &ExtractLimits {
            max_component_bytes: 7,
            ..ExtractLimits::DEFAULT
        },
    );
    assert_eq!(
        rejects_with(
            &image,
            &ExtractLimits {
                max_component_bytes: 6,
                ..ExtractLimits::DEFAULT
            },
            "extract.over_limit.component_bytes",
        ),
        0
    );
}

#[test]
fn the_depth_limit_holds_at_its_boundary() {
    let image = one_file(b"Payload/ID-1/x.pdf");

    accepts(
        &image,
        &ExtractLimits {
            max_depth: 3,
            ..ExtractLimits::DEFAULT
        },
    );
    assert_eq!(
        rejects_with(
            &image,
            &ExtractLimits {
                max_depth: 2,
                ..ExtractLimits::DEFAULT
            },
            "extract.over_limit.depth",
        ),
        0
    );
}

// --- diagnostics -------------------------------------------------------------

#[test]
fn error_display_carries_codes_and_numbers_but_no_entry_name() {
    let image = Archive::of(vec![
        Entry::stored(b"mimetype", b"application/OCD+ZIP"),
        Entry::stored(b"Payload/canary-name.pdf", b"attachment"),
    ])
    .build();
    let inventory = archive::inventory(&image, &Limits::DEFAULT).expect("archive is readable");
    let error = extract::plan(
        &inventory,
        &ExtractLimits {
            max_files: 1,
            ..ExtractLimits::DEFAULT
        },
    )
    .expect_err("the plan is over the file limit");

    let text = error.to_string();
    assert_eq!(
        text,
        "extract.over_limit.files (limit 1, observed 2) at entry 1"
    );
    assert!(!text.contains("canary"));
    assert!(!text.contains("Payload"));
}

#[test]
fn a_planning_error_is_a_standard_error() {
    let image = Archive::of(vec![
        Entry::stored(b"dev", b"").with_unix_mode(MODE_SYMLINK),
    ])
    .build();
    let inventory = archive::inventory(&image, &Limits::DEFAULT).expect("archive is readable");
    let error =
        extract::plan(&inventory, &ExtractLimits::DEFAULT).expect_err("a symlink is refused");
    let boxed: Box<dyn std::error::Error> = Box::new(error);

    assert_eq!(boxed.to_string(), "extract.unsupported.link at entry 0");
}
