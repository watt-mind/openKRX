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
//! touches anything that was already there. The undo takes the same arm the
//! creation took: on Unix each removal is an `unlinkat` beneath the
//! destination descriptor, so it reaches the directory this run wrote into
//! rather than whatever now answers to the destination's name. The command then exits with the
//! failure's own category, and reports how many paths were removed and how
//! many could not be. Success is never reported over partial output.
//!
//! **Residual limitation.** A crash — a signal, a power loss, a killed
//! process — cannot run the undo pass, so it leaves the partial files and the
//! marker behind. That is what the marker is for: a destination containing it
//! is incomplete, and the next run refuses to add to it rather than mixing two
//! runs' output. Clearing it is a deliberate human act.
//!
//! **Race assumptions.** How much of the check-to-create window is closed
//! depends on the platform, and each run says which it got.
//!
//! On **Unix** the destination is opened once, after preflight, and no
//! absolute path is resolved again: every creation and every removal is made
//! relative to that descriptor. On Linux with `openat2(2)` — kernel 5.6 and
//! later — the kernel resolves each path under `RESOLVE_BENEATH`,
//! `RESOLVE_NO_SYMLINKS` and `RESOLVE_NO_MAGICLINKS`; on every other Unix
//! target the components are walked one at a time with
//! `O_DIRECTORY | O_NOFOLLOW`. Either way a component swapped for a symbolic
//! link between preflight and the write is refused rather than followed. See
//! [`unix_fd`].
//!
//! On **Windows** — and on a Linux kernel without `openat2`, which reports
//! `path_resolution_fallback: true` — the destination is trusted not to be
//! modified by another principal while the command runs. Exclusive creation
//! and the post-creation `symlink_metadata` checks defend against what is
//! *already* at the destination — an existing file, a symbolic link, a
//! reparse point — and not against an attacker with concurrent write access
//! to it, who can win the window between a check and the operation that
//! follows. The per-platform position is in `docs/architecture.md`, and the
//! residual risk in `SECURITY.md`.

pub mod cleanup;
pub mod preflight;
pub mod resolver;
#[cfg(unix)]
pub mod unix_fd;
pub mod writer;

#[cfg(all(test, unix))]
mod tests;

use std::path::Path;

use openkrx_core::archive::ArchiveInventory;
use openkrx_core::extract::{self, ExtractLimits};

use crate::commands::extract::ExtractData;
use crate::exit::Failure;
use cleanup::{Cleanup, Ledger};
use resolver::Resolver;

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
    run_between(inventory, destination, &mut || (), Resolver::open)
}

/// The whole run, with the two seams a race test needs and a caller does not.
///
/// `between` is called once, after preflight and after the planned directories
/// exist, and immediately before the first file is written: that is the window
/// `openat2` closes, and a test plants a symbolic link through it. `resolver`
/// chooses the path-resolution arm, so a test can force the fallback branch on
/// a kernel that does have `openat2`. Choosing it can itself refuse the run —
/// a destination that is no longer the real directory preflight accepted is a
/// refusal, never a quiet fallback.
///
/// Both are ordinary parameters of a private function. Nothing in the public
/// surface of this module mentions them, nothing is gated on `cfg(test)` or on
/// a feature, and [`run`] passes a closure that does nothing and the real
/// chooser — so the shipped path is the tested path.
fn run_between(
    inventory: &ArchiveInventory<'_>,
    destination: &Path,
    between: &mut dyn FnMut(),
    resolver: impl Fn(&Path) -> Result<Resolver, Failure>,
) -> Result<ExtractData, Refusal> {
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

    // After this point no absolute path is resolved again on the Linux arm:
    // the destination is held open, and every planned path is resolved by the
    // kernel relative to that descriptor. Opening it is the last check of the
    // destination itself, and a failure is a refusal: continuing on the
    // portable arm would hand a destination that had just been replaced to
    // the code path that resolves straight through the replacement.
    let resolver = resolver(destination).map_err(refuse)?;
    let mut ledger = Ledger::new();
    match commit(
        inventory,
        destination,
        &plan,
        &resolver,
        &mut ledger,
        between,
    ) {
        Ok(data) => Ok(data),
        Err(failure) => Err(Refusal {
            failure,
            cleanup: ledger.undo(&resolver, destination),
        }),
    }
}

/// The writing half: marker, directories, files, marker removed.
fn commit(
    inventory: &ArchiveInventory<'_>,
    destination: &Path,
    plan: &extract::ExtractionPlan,
    resolver: &Resolver,
    ledger: &mut Ledger,
    between: &mut dyn FnMut(),
) -> Result<ExtractData, Failure> {
    let marker = writer::marker(resolver, destination, ledger)?;
    let mut data = writer::write(inventory, destination, plan, resolver, ledger, between)?;
    writer::remove_marker(resolver, destination, ledger, marker)?;
    data.marker_removed = true;
    Ok(data)
}
