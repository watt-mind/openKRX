//! Plans the extraction planner produces, and the facts it reports.
//!
//! Nothing in this file touches a filesystem, because nothing in the planner
//! can: a plan is a value describing what an extraction would create.

mod support;

use openkrx_core::archive::EntryKind;
use openkrx_core::extract::{self, ExtractLimits};
use openkrx_core::{Limits, archive};
use support::{Archive, Entry, marker_archive, pseudo_random};

/// Unix `st_mode` for a regular file with the usual permissions.
const MODE_REGULAR: u32 = 0o100_644;
/// Unix `st_mode` for a directory.
const MODE_DIRECTORY: u32 = 0o040_755;

#[track_caller]
fn plan_of(image: &[u8]) -> extract::ExtractionPlan {
    let inventory = archive::inventory(image, &Limits::DEFAULT).expect("archive is readable");
    extract::plan(&inventory, &ExtractLimits::DEFAULT).expect("plan is producible")
}

/// The destination paths of a plan, joined for readable assertions.
fn joined(plan: &extract::ExtractionPlan) -> Vec<String> {
    plan.items()
        .iter()
        .map(|item| item.components().join("/"))
        .collect()
}

// --- what a plan contains ----------------------------------------------------

#[test]
fn a_krx_shaped_package_plans_its_files_in_inventory_order() {
    let image = Archive::of(vec![
        Entry::stored(b"mimetype", b"application/OCD+ZIP"),
        Entry::deflated(b"Metalayer/KULDEMENY_META.xml", b"<KULDEMENY/>"),
        Entry::stored(b"Payload/ID-1/x.pdf", b"first attachment"),
        Entry::stored(b"Payload/ID-2/y.pdf", b"second attachment"),
    ])
    .build();
    let plan = plan_of(&image);

    assert_eq!(
        joined(&plan),
        [
            "mimetype",
            "Metalayer/KULDEMENY_META.xml",
            "Payload/ID-1/x.pdf",
            "Payload/ID-2/y.pdf",
        ]
    );
    assert_eq!(
        plan.items()
            .iter()
            .map(|item| item.entry_index())
            .collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    assert_eq!(
        plan.directories(),
        [
            vec!["Metalayer".to_string()],
            vec!["Payload".to_string()],
            vec!["Payload".to_string(), "ID-1".to_string()],
            vec!["Payload".to_string(), "ID-2".to_string()],
        ]
    );
    assert_eq!(plan.len(), 4);
    assert!(!plan.is_empty());
    assert_eq!(plan.limits_applied(), ExtractLimits::DEFAULT);
}

#[test]
fn every_item_carries_the_declared_and_the_counted_size() {
    let payload = pseudo_random(4096);
    let image = Archive::of(vec![
        Entry::stored(b"a.bin", &payload),
        Entry::deflated(b"b.txt", b"text"),
    ])
    .build();
    let plan = plan_of(&image);

    assert_eq!(plan.items()[0].declared_size(), payload.len() as u64);
    assert_eq!(plan.items()[0].decoded_size(), payload.len() as u64);
    assert_eq!(plan.items()[1].decoded_size(), 4);
    assert_eq!(plan.total_bytes(), payload.len() as u64 + 4);
}

#[test]
fn a_directory_marker_produces_no_item_but_its_children_do() {
    let image = Archive::of(vec![
        Entry::stored(b"Payload/", b""),
        Entry::stored(b"Payload/ID-1/", b""),
        Entry::stored(b"Payload/ID-1/x.pdf", b"attachment"),
        Entry::stored(b"Empty/", b""),
    ])
    .build();
    let plan = plan_of(&image);

    assert_eq!(joined(&plan), ["Payload/ID-1/x.pdf"]);
    assert_eq!(
        plan.directories(),
        [
            vec!["Payload".to_string()],
            vec!["Payload".to_string(), "ID-1".to_string()],
        ],
        "an empty directory is deliberately not materialised"
    );
}

#[test]
fn an_archive_of_directory_markers_alone_plans_nothing() {
    let image = Archive::of(vec![Entry::stored(b"Payload/", b"")]).build();
    let plan = plan_of(&image);

    assert!(plan.is_empty());
    assert_eq!(plan.total_bytes(), 0);
    assert!(plan.directories().is_empty());
}

#[test]
fn a_plan_holds_no_path_separator_and_no_absolute_path() {
    let image = marker_archive();
    let plan = plan_of(&image);

    for item in plan.items() {
        for component in item.components() {
            assert!(!component.contains('/'), "component holds a separator");
            assert!(!component.contains('\\'), "component holds a separator");
        }
        assert!(
            !item.components()[0].is_empty(),
            "a plan never starts at a root"
        );
    }
}

#[test]
fn planning_the_same_inventory_twice_produces_the_same_plan() {
    let image = Archive::of(vec![
        Entry::stored(b"mimetype", b"application/OCD+ZIP"),
        Entry::stored(b"Payload/ID-1/x.pdf", b"attachment"),
        Entry::stored(b"Payload/", b""),
    ])
    .build();
    let inventory = archive::inventory(&image, &Limits::DEFAULT).expect("archive is readable");

    let first = extract::plan(&inventory, &ExtractLimits::DEFAULT).expect("plan is producible");
    let second = extract::plan(&inventory, &ExtractLimits::DEFAULT).expect("plan is producible");
    assert_eq!(first, second);
}

// --- entry kinds -------------------------------------------------------------

#[test]
fn entry_kinds_are_read_from_the_central_directory_fields() {
    let image = Archive::of(vec![
        Entry::stored(b"regular", b"x").with_unix_mode(MODE_REGULAR),
        Entry::stored(b"unix-dir", b"").with_unix_mode(MODE_DIRECTORY),
        Entry::stored(b"dos-dir", b"").with_dos_attributes(0x10),
        Entry::stored(b"dos-file", b"x").with_dos_attributes(0x20),
        Entry::stored(b"marker/", b""),
        Entry::stored(b"foreign", b"x").with_host_system(19),
    ])
    .build();
    let inventory = archive::inventory(&image, &Limits::DEFAULT).expect("archive is readable");
    let kinds: Vec<EntryKind> = inventory.entries().iter().map(|e| e.kind()).collect();

    assert_eq!(
        kinds,
        [
            EntryKind::RegularFile,
            EntryKind::DirectoryMarker,
            EntryKind::DirectoryMarker,
            EntryKind::RegularFile,
            EntryKind::DirectoryMarker,
            EntryKind::Unknown,
        ]
    );
    assert_eq!(inventory.entries()[0].version_made_by(), (3 << 8) | 20);
    assert_eq!(
        inventory.entries()[0].external_attributes(),
        MODE_REGULAR << 16
    );

    let plan = extract::plan(&inventory, &ExtractLimits::DEFAULT).expect("plan is producible");
    assert_eq!(joined(&plan), ["regular", "dos-file", "foreign"]);
}

#[test]
fn an_entry_from_an_unmapped_host_is_planned_as_an_ordinary_file() {
    let image = Archive::of(vec![
        Entry::stored(b"Payload/x.pdf", b"attachment").with_host_system(19),
    ])
    .build();
    let plan = plan_of(&image);

    assert_eq!(joined(&plan), ["Payload/x.pdf"]);
}

// --- limits ------------------------------------------------------------------

#[test]
fn extract_limits_default_is_the_documented_table() {
    let limits = ExtractLimits::DEFAULT;
    assert_eq!(limits.max_files, 256);
    assert_eq!(limits.max_total_bytes, 128 * 1024 * 1024);
    assert_eq!(limits.max_path_bytes, 1024);
    assert_eq!(limits.max_component_bytes, 255);
    assert_eq!(limits.max_depth, 16);
    assert_eq!(ExtractLimits::default(), limits);
}

// --- sweeps ------------------------------------------------------------------

/// Archive shapes the archive tests already build, plus the KRX-shaped one.
fn fixture_images() -> Vec<Vec<u8>> {
    vec![
        marker_archive(),
        Archive::default().build(),
        Archive::of(vec![Entry::stored(b"a.txt", b"payload")]).build(),
        Archive::of(vec![
            Entry::stored(b"mimetype", b"application/OCD+ZIP"),
            Entry::deflated(b"Metalayer/KULDEMENY_META.xml", b"<KULDEMENY/>"),
            Entry::stored(b"Payload/ID-1/x.pdf", b"attachment"),
        ])
        .build(),
        Archive::of(vec![
            Entry::stored(b"Payload/", b""),
            Entry::stored(b"Payload/x", b"one"),
            Entry::deflated(b"Payload/y", &pseudo_random(1024)),
        ])
        .build(),
        Archive::of(vec![Entry::stored(b"zero", b"").with_descriptor(true)]).build(),
    ]
}

#[test]
fn planning_every_fixture_inventory_never_panics() {
    for image in fixture_images() {
        let Ok(inventory) = archive::inventory(&image, &Limits::DEFAULT) else {
            continue;
        };
        let _ = extract::plan(&inventory, &ExtractLimits::DEFAULT);
    }
}

#[test]
fn a_name_mutation_sweep_never_panics_and_never_plans_an_unsafe_path() {
    let base = b"Payload/ID-1/x.pdf";
    for position in 0..base.len() {
        for byte in 0..=u8::MAX {
            let mut name = base.to_vec();
            name[position] = byte;
            let image = Archive::of(vec![
                Entry::stored(b"mimetype", b"application/OCD+ZIP"),
                Entry::stored(&name, b"attachment"),
            ])
            .build();
            let Ok(inventory) = archive::inventory(&image, &Limits::DEFAULT) else {
                continue;
            };
            let Ok(plan) = extract::plan(&inventory, &ExtractLimits::DEFAULT) else {
                continue;
            };
            for item in plan.items() {
                for component in item.components() {
                    assert!(!component.is_empty());
                    assert!(component != "." && component != "..");
                    assert!(!component.contains('/') && !component.contains(':'));
                    assert!(!component.chars().any(char::is_control));
                }
            }
        }
    }
}
