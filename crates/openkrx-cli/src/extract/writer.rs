//! Creating the marker, the directories and the files, in plan order.
//!
//! Every creation here is exclusive, and every one of them goes through the
//! [`Resolver`], which owns how a path is resolved: beneath a directory
//! descriptor on Unix, and by path on Windows. Nothing in this module opens a
//! path itself, so neither arm can drift from the other.
//!
//! Directories are created one level at a time, never `create_dir_all`, so no
//! parent is invented behind the caller's back. Files are created with
//! `O_CREAT | O_EXCL | O_NOFOLLOW`, or the `CREATE_NEW` that stands for it on
//! Windows: it fails rather than truncating, and it does not follow a symbolic
//! link at the leaf. Preflight already refused an existing path; exclusive
//! creation is the second, narrower guard that closes the window between the
//! two.
//!
//! Nothing is copied from the archive except the bytes. No mode bit, no
//! timestamp and no attribute is carried across: a package is untrusted input,
//! and the default umask — or the destination's inherited Windows ACL — is a
//! deliberate choice rather than a loss. Payload bytes come from
//! `ArchiveInventory::entry_bytes`, which decodes one entry under the archive
//! limits and checks its CRC-32 (corruption, not authenticity).
//!
//! Each successful creation is recorded in the [`Ledger`] before the next step
//! begins, so a failure at any point can undo exactly this run's work. What is
//! recorded is the path's components relative to the destination, never a
//! joined path: the undo pass resolves them through the same [`Resolver`].

use std::io::Write;
use std::path::Path;

use openkrx_core::{ExtractionPlan, archive::ArchiveInventory};

use super::cleanup::{Ledger, Recorded};
use super::resolver::{Directory, Resolver};
use crate::commands::extract::{ExtractData, WrittenView};
use crate::exit::{Failure, OUTPUT_IO};

/// Create the marker that says an extraction into this destination is running.
///
/// # Errors
///
/// `output.partial_marker_present` when another run got there first, and
/// `output.io` when the destination refuses the file.
pub fn marker(
    resolver: &Resolver,
    destination: &Path,
    ledger: &mut Ledger,
) -> Result<Recorded, Failure> {
    resolver.marker(destination)?;
    Ok(ledger.file(vec![super::MARKER_NAME.to_owned()]))
}

/// Remove the marker, which is the last step of a successful run.
///
/// # Errors
///
/// `output.io`. The run has written every file it planned by this point, so
/// the failure is reported rather than ignored: a destination still carrying
/// the marker must be read as incomplete.
pub fn remove_marker(
    resolver: &Resolver,
    destination: &Path,
    ledger: &mut Ledger,
    recorded: Recorded,
) -> Result<(), Failure> {
    resolver.remove_marker(destination)?;
    ledger.forget(recorded);
    Ok(())
}

/// Create every planned directory, parent before child, then every file.
///
/// `between` is called once, after the last directory and before the first
/// file. It does nothing in a real run: it is the seam the race test plants a
/// symbolic link through, so that the window this writer has to survive is
/// exercised rather than described.
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
    resolver: &Resolver,
    ledger: &mut Ledger,
    between: &mut dyn FnMut(),
) -> Result<ExtractData, Failure> {
    let directories_created = directories(destination, plan, resolver, ledger)?;
    between();
    let mut data = ExtractData {
        files_written: 0,
        directories_created,
        bytes_written: 0,
        items: Vec::with_capacity(plan.len()),
        marker_removed: false,
        path_resolution_fallback: resolver.fell_back(),
    };
    for item in plan.items() {
        let entry = item.entry_index();
        let bytes = inventory.entry_bytes(entry)?;
        let mut file = resolver.file(destination, item.components(), entry)?;
        ledger.file(item.components().to_vec());
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
/// precedes its child, which is why one creation per entry suffices. A
/// directory that already exists was accepted by preflight as a real
/// directory, and is not counted or recorded: this run did not create it, so
/// this run must not remove it.
///
/// A refusal that *did* create the directory first — the portable arm reading
/// back a link someone put there between the creation and the check — records
/// it before returning, so the undo pass removes it. The count is not raised:
/// it reports a completed run, and this run is about to fail.
fn directories(
    destination: &Path,
    plan: &ExtractionPlan,
    resolver: &Resolver,
    ledger: &mut Ledger,
) -> Result<u32, Failure> {
    let mut created = 0_u32;
    for components in plan.directories() {
        match resolver.directory(destination, components) {
            Ok(Directory::Created) => {
                ledger.directory(components.clone());
                created = created.saturating_add(1);
            }
            Ok(Directory::AlreadyThere) => {}
            Err(error) => {
                if error.created {
                    ledger.directory(components.clone());
                }
                return Err(error.failure);
            }
        }
    }
    Ok(created)
}
