//! Everything decided about the filesystem before a single byte is written.
//!
//! The planner has already refused every destination path that could not be
//! written safely on any of the three target platforms. What it cannot know is
//! what is *already* at the destination, and that is this module's whole job:
//! the destination itself, the marker an interrupted run leaves behind, and,
//! for every planned path, whether something is in the way.
//!
//! Two rules are held here.
//!
//! - **No overwrite.** A planned file path must not exist in any form —
//!   file, directory, link or anything else. `symlink_metadata` answers that
//!   without following a link, so a dangling symlink counts as existing rather
//!   than as free space.
//! - **No link in the path.** Every existing ancestor of a planned path,
//!   inside the destination, must be a real directory. A symbolic link or a
//!   Windows reparse point in that position would place output outside the
//!   destination the caller chose, which is exactly the confinement
//!   `SECURITY.md` requires.
//!
//! Neither check is a defence against a concurrent attacker holding write
//! access to the destination; the race assumptions are stated in
//! `docs/architecture.md#extraction-output`. They defend against what is
//! already there, and the writer re-checks after each directory it creates.
//!
//! Nothing here writes, and no diagnostic carries a path: a refusal reports
//! its stable code and, where the failure belongs to one planned file, that
//! entry's central-directory index.

use std::fs::Metadata;
use std::path::{Path, PathBuf};

use openkrx_core::ExtractionPlan;

use crate::exit::{
    Failure, OUTPUT_DESTINATION_MISSING, OUTPUT_DESTINATION_NOT_A_DIRECTORY,
    OUTPUT_DESTINATION_SYMLINK, OUTPUT_EXISTS, OUTPUT_IO, OUTPUT_NOT_A_DIRECTORY,
    OUTPUT_PARTIAL_MARKER_PRESENT, OUTPUT_SYMLINK_IN_PATH,
};

/// Whether a metadata record describes something that must never be walked
/// through or written over as if it were a plain directory or file.
///
/// A symbolic link is a symbolic link on every supported platform. Windows
/// adds junctions and other reparse points, which `is_symlink` does not always
/// report, so the `FILE_ATTRIBUTE_REPARSE_POINT` bit is read directly. Both
/// use the safe standard-library accessors; the workspace forbids `unsafe`.
#[must_use]
pub fn is_link(metadata: &Metadata) -> bool {
    metadata.file_type().is_symlink() || reparse_point(metadata)
}

/// Windows: the entry carries `FILE_ATTRIBUTE_REPARSE_POINT`.
#[cfg(windows)]
fn reparse_point(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    /// `FILE_ATTRIBUTE_REPARSE_POINT`, as `winnt.h` defines it.
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;

    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

/// Elsewhere: reparse points do not exist, and `is_symlink` is the whole rule.
#[cfg(not(windows))]
const fn reparse_point(_metadata: &Metadata) -> bool {
    false
}

/// Join destination components onto `destination`.
///
/// Every component came from `openkrx_core::extract::plan`, which has already
/// refused an empty component, `.`, `..`, a control character, a colon and a
/// Windows reserved device name. The archive layer refused a backslash and an
/// absolute name before that. `Path::push` therefore cannot leave the
/// destination, on any platform, and no normalisation is performed here.
#[must_use]
pub fn join(destination: &Path, components: &[String]) -> PathBuf {
    let mut path = destination.to_path_buf();
    for component in components {
        path.push(component);
    }
    path
}

/// The destination must already exist, as a real directory, and not be a link.
///
/// `extract` never creates its destination. Creating one would mean deciding
/// where, with which parents and with which permissions, and a typo in the
/// argument would silently produce a new tree instead of a refusal.
///
/// # Errors
///
/// `output.destination_missing`, `output.destination_not_a_directory`,
/// `output.destination_symlink`, or `output.io` when the destination cannot be
/// examined at all.
pub fn destination(destination: &Path) -> Result<(), Failure> {
    let metadata = std::fs::symlink_metadata(destination).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Failure::output(OUTPUT_DESTINATION_MISSING)
        } else {
            Failure::output(OUTPUT_IO)
        }
    })?;
    if is_link(&metadata) {
        return Err(Failure::output(OUTPUT_DESTINATION_SYMLINK));
    }
    if !metadata.is_dir() {
        return Err(Failure::output(OUTPUT_DESTINATION_NOT_A_DIRECTORY));
    }
    Ok(())
}

/// Refuse to start when an earlier run's marker is still there.
///
/// The marker means a previous extraction into this destination did not finish
/// and its partial output may still be present. Writing over that would mix
/// two runs' files, and the caller would have no way to tell which is which.
///
/// # Errors
///
/// `output.partial_marker_present` when anything exists at the marker path.
pub fn marker(path: &Path) -> Result<(), Failure> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Err(Failure::output(OUTPUT_PARTIAL_MARKER_PRESENT)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(Failure::output(OUTPUT_IO)),
    }
}

/// Check every planned path against what is already at the destination.
///
/// Each item is walked component by component: every ancestor that exists must
/// be a real directory, and the leaf must not exist at all. The planner's
/// directory list holds exactly those ancestors, so walking the items covers
/// the directories too, and each check is attributed to the entry whose output
/// it concerns.
///
/// # Errors
///
/// `output.symlink_in_path`, `output.not_a_directory` or `output.exists`, each
/// carrying the entry index and no path.
pub fn plan_paths(destination: &Path, plan: &ExtractionPlan) -> Result<(), Failure> {
    for item in plan.items() {
        let entry = item.entry_index();
        let components = item.components();
        let (leaf, ancestors) = components
            .split_last()
            .expect("a planned path has at least one component");
        let mut path = destination.to_path_buf();
        for ancestor in ancestors {
            path.push(ancestor);
            existing_directory(&path, entry)?;
        }
        path.push(leaf);
        absent(&path, entry)?;
    }
    Ok(())
}

/// An ancestor inside the destination: absent, or a real directory.
fn existing_directory(path: &Path, entry: u32) -> Result<(), Failure> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(Failure::output_at(OUTPUT_IO, entry)),
    };
    if is_link(&metadata) {
        return Err(Failure::output_at(OUTPUT_SYMLINK_IN_PATH, entry));
    }
    if !metadata.is_dir() {
        return Err(Failure::output_at(OUTPUT_NOT_A_DIRECTORY, entry));
    }
    Ok(())
}

/// A planned file path: nothing may be there, in any form.
fn absent(path: &Path, entry: u32) -> Result<(), Failure> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Err(Failure::output_at(OUTPUT_EXISTS, entry)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(Failure::output_at(OUTPUT_IO, entry)),
    }
}
