//! Creating the marker, the directories and the files, in plan order.
//!
//! Every creation here is exclusive. Directories are created one level at a
//! time with `create_dir`, never `create_dir_all`, so no parent is invented
//! behind the caller's back, and each one is re-examined with
//! `symlink_metadata` after creation. Files are created with
//! `OpenOptions::create_new`, which is `O_EXCL` on Unix and
//! `CREATE_NEW` on Windows: it fails rather than truncating, and it does not
//! follow a symbolic link at the leaf. Preflight already refused an existing
//! path; `create_new` is the second, narrower guard that closes the window
//! between the two.
//!
//! Nothing is copied from the archive except the bytes. No mode bit, no
//! timestamp and no attribute is carried across: a package is untrusted input,
//! and the default umask — or the destination's inherited Windows ACL — is a
//! deliberate choice rather than a loss. Payload bytes come from
//! `ArchiveInventory::entry_bytes`, which decodes one entry under the archive
//! limits and checks its CRC-32 (corruption, not authenticity).
//!
//! Each successful creation is recorded in the [`Ledger`] before the next step
//! begins, so a failure at any point can undo exactly this run's work.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use openkrx_core::{ExtractionPlan, archive::ArchiveInventory};

use super::cleanup::Ledger;
use super::preflight::{is_link, join};
use crate::commands::extract::{ExtractData, WrittenView};
use crate::exit::{
    Failure, OUTPUT_IO, OUTPUT_NOT_A_DIRECTORY, OUTPUT_PARTIAL_MARKER_PRESENT,
    OUTPUT_SYMLINK_IN_PATH,
};

/// Create the marker that says an extraction into this destination is running.
///
/// # Errors
///
/// `output.partial_marker_present` when another run got there first, and
/// `output.io` when the destination refuses the file.
pub fn marker(path: &Path, ledger: &mut Ledger) -> Result<(), Failure> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                Failure::output(OUTPUT_PARTIAL_MARKER_PRESENT)
            } else {
                Failure::output(OUTPUT_IO)
            }
        })?;
    ledger.file(path.to_path_buf());
    Ok(())
}

/// Remove the marker, which is the last step of a successful run.
///
/// # Errors
///
/// `output.io`. The run has written every file it planned by this point, so
/// the failure is reported rather than ignored: a destination still carrying
/// the marker must be read as incomplete.
pub fn remove_marker(path: &Path, ledger: &mut Ledger) -> Result<(), Failure> {
    std::fs::remove_file(path).map_err(|_| Failure::output(OUTPUT_IO))?;
    ledger.forget(path);
    Ok(())
}

/// Create every planned directory, parent before child, then every file.
///
/// # Errors
///
/// `output.io` or `output.symlink_in_path` from a directory, and the same plus
/// any `archive.*` code from decoding an entry's bytes. Every failure leaves
/// the ledger holding exactly what had been created, for the caller to undo.
pub fn write(
    inventory: &ArchiveInventory<'_>,
    destination: &Path,
    plan: &ExtractionPlan,
    ledger: &mut Ledger,
) -> Result<ExtractData, Failure> {
    let directories_created = directories(destination, plan, ledger)?;
    let mut data = ExtractData {
        files_written: 0,
        directories_created,
        bytes_written: 0,
        items: Vec::with_capacity(plan.len()),
        marker_removed: false,
    };
    for item in plan.items() {
        let entry = item.entry_index();
        let path = join(destination, item.components());
        let bytes = inventory.entry_bytes(entry)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|_| Failure::output_at(OUTPUT_IO, entry))?;
        ledger.file(path);
        file.write_all(&bytes)
            .map_err(|_| Failure::output_at(OUTPUT_IO, entry))?;
        data.files_written = data.files_written.saturating_add(1);
        data.bytes_written = data.bytes_written.saturating_add(bytes.len() as u64);
        data.items.push(WrittenView {
            entry_index: entry,
            path: item.components().join("/"),
            bytes: bytes.len() as u64,
        });
    }
    Ok(data)
}

/// Create the plan's directories, and count the ones this run created.
///
/// The plan lists every implicit parent, deduplicated and sorted so a parent
/// precedes its child, which is why one `create_dir` per entry suffices. A
/// directory that already exists was accepted by preflight as a real
/// directory, and is not counted or recorded: this run did not create it, so
/// this run must not remove it.
fn directories(
    destination: &Path,
    plan: &ExtractionPlan,
    ledger: &mut Ledger,
) -> Result<u32, Failure> {
    let mut created = 0_u32;
    for components in plan.directories() {
        let path = join(destination, components);
        match std::fs::create_dir(&path) {
            Ok(()) => {
                ledger.directory(path.clone());
                created = created.saturating_add(1);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(Failure::output(OUTPUT_IO)),
        }
        // Re-read after creating it. The check costs one `lstat` per planned
        // directory and closes the case where what is at the path now is not
        // what preflight saw: it is the writer's own confirmation that it is
        // about to descend into a real directory rather than through a link.
        let metadata = std::fs::symlink_metadata(&path).map_err(|_| Failure::output(OUTPUT_IO))?;
        if is_link(&metadata) {
            return Err(Failure::output(OUTPUT_SYMLINK_IN_PATH));
        }
        if !metadata.is_dir() {
            return Err(Failure::output(OUTPUT_NOT_A_DIRECTORY));
        }
    }
    Ok(created)
}
