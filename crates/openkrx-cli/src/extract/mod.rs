//! Protected filesystem output: the only code in openKRX that writes.
//!
//! `openkrx_core::extract::plan` decides *what* an extraction would create,
//! as a pure function of the archive inventory. This module does the other
//! half, and nothing more: it joins that plan onto a caller-selected
//! destination, checks what is already there, writes, and cleans up after
//! itself if anything goes wrong. It adds no rule about names, kinds,
//! collisions or ceilings — those are the planner's, and are enforced before
//! this module is reached.
//!
//! The run has four phases, in this order, and each one must finish before the
//! next begins:
//!
//! 1. **Plan.** The whole plan or nothing: a planning refusal rejects the
//!    package rather than extracting a reduced version of it.
//! 2. **Preflight.** The destination, the marker, and every planned path
//!    against what is already at the destination. Nothing has been written
//!    when a preflight refusal is reported.
//! 3. **Write.** The marker, then the directories parent-first, then the
//!    files, each created exclusively and recorded as this run created it.
//! 4. **Commit.** Removing the marker is the last step. A destination that
//!    still holds `.openkrx-extract.partial` was not finished.
//!
//! **Failure policy.** Any failure after the first write undoes this run's
//! work — files, directories and the marker, in reverse order — and never
//! touches anything that was already there. The command then exits with the
//! failure's own category, and reports how many paths were removed and how
//! many could not be. Success is never reported over partial output.
//!
//! **Residual limitation.** A crash — a signal, a power loss, a killed
//! process — cannot run the undo pass, so it leaves the partial files and the
//! marker behind. That is what the marker is for: a destination containing it
//! is incomplete, and the next run refuses to add to it rather than mixing two
//! runs' output. Clearing it is a deliberate human act.
//!
//! **Race assumptions.** The destination is trusted not to be modified by
//! another principal while the command runs. `create_new` and the
//! post-creation `symlink_metadata` checks defend against what is *already*
//! at the destination — an existing file, a symbolic link, a Windows reparse
//! point — and not against an attacker with concurrent write access to it,
//! who can win the window between a check and the operation that follows.
//! Closing that window needs `openat2`/`RESOLVE_BENEATH` or the equivalent
//! per-platform primitive, which is deliberately deferred and recorded in
//! `SECURITY.md`.

pub mod cleanup;
pub mod preflight;
pub mod writer;

use std::path::Path;

use openkrx_core::archive::ArchiveInventory;
use openkrx_core::extract::{self, ExtractLimits};

use crate::commands::extract::ExtractData;
use crate::exit::Failure;
use cleanup::{Cleanup, Ledger};

/// The file that says an extraction into this destination is unfinished.
///
/// Created before the first write and removed after the last, so its presence
/// is the one durable signal a crashed run leaves behind. Nothing stops a
/// package from declaring an entry of this name: the planner has no opinion
/// about it, and the marker is created first, so the no-clobber rule refuses
/// such a package rather than letting two things share the path.
pub const MARKER_NAME: &str = ".openkrx-extract.partial";

/// A failed extraction: why it failed, and what the undo pass did about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Refusal {
    /// The failure, carrying its stable code, category and entry index.
    pub failure: Failure,
    /// How many of this run's paths were removed, and how many were not.
    pub cleanup: Cleanup,
}

/// Extract `inventory` into `destination`, or refuse and leave it as found.
///
/// # Errors
///
/// Returns a [`Refusal`]. Its `failure` carries the stable code and the exit
/// category — `extract.*` from the planner, `output.*` from the destination
/// or a write, `archive.*` from decoding an entry — and its `cleanup` says
/// what the undo pass removed. Before the first write, `cleanup` is
/// [`Cleanup::NONE`], because there was nothing to undo.
pub fn run(inventory: &ArchiveInventory<'_>, destination: &Path) -> Result<ExtractData, Refusal> {
    let refuse = |failure: Failure| Refusal {
        failure,
        cleanup: Cleanup::NONE,
    };
    let plan = extract::plan(inventory, &ExtractLimits::DEFAULT)
        .map_err(|error| refuse(Failure::from(error)))?;
    preflight::destination(destination).map_err(refuse)?;
    let marker = destination.join(MARKER_NAME);
    preflight::marker(&marker).map_err(refuse)?;
    preflight::plan_paths(destination, &plan).map_err(refuse)?;

    let mut ledger = Ledger::new();
    match commit(inventory, destination, &marker, &plan, &mut ledger) {
        Ok(data) => Ok(data),
        Err(failure) => Err(Refusal {
            failure,
            cleanup: ledger.undo(),
        }),
    }
}

/// The writing half: marker, directories, files, marker removed.
fn commit(
    inventory: &ArchiveInventory<'_>,
    destination: &Path,
    marker: &Path,
    plan: &extract::ExtractionPlan,
    ledger: &mut Ledger,
) -> Result<ExtractData, Failure> {
    writer::marker(marker, ledger)?;
    let mut data = writer::write(inventory, destination, plan, ledger)?;
    writer::remove_marker(marker, ledger)?;
    data.marker_removed = true;
    Ok(data)
}
