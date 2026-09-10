//! The ledger of what this run created, and the reverse-order undo.
//!
//! Extraction has no atomic commit: files are created one by one, in plan
//! order, directly in the destination the caller chose. The compensating rule
//! is that **only what this run created is ever removed**. Every successful
//! `create_dir` and every successful `create_new` file is recorded here, in
//! creation order, and a failure walks the record backwards removing exactly
//! those paths.
//!
//! Nothing pre-existing can be removed by construction. A directory that
//! already existed was never recorded, so it is never a candidate; a directory
//! this run created is removed with `rmdir`, never recursively, so a file
//! someone else put inside it in the meantime keeps the directory alive
//! instead of being deleted with it.
//!
//! What is recorded is a path's **components, relative to the destination**,
//! and never a joined absolute path. The undo pass hands them back to the
//! [`Resolver`], which removes each one the same way it created it: beneath
//! the destination descriptor on Unix, and by name on Windows. A ledger of
//! absolute paths would have had the undo pass resolve every one of them
//! again, by name, at the moment a run is failing — which is the moment a
//! destination is least likely to still be what it was.
//!
//! The counts the pass produces carry no path: [`Cleanup::removed`] is how
//! many of this run's paths are gone, and [`Cleanup::left_in_place`] how many
//! could not be removed and are still there. A caller reads the second number
//! as "the destination is not as it was found".

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::resolver::{Kind, Resolver};

/// What the cleanup pass did, in counts. Never a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Cleanup {
    /// Paths this run created that are gone again.
    pub removed: u32,
    /// Paths this run created that could not be removed and are still there.
    pub left_in_place: u32,
}

impl Cleanup {
    /// Nothing had been created when the failure happened.
    pub const NONE: Self = Self {
        removed: 0,
        left_in_place: 0,
    };
}

/// Where a thing this run created lives, and therefore how it is reached
/// again.
///
/// Two shapes, because two kinds of writer share this ledger. An extraction
/// creates many paths under one destination it holds open, and records what
/// it created relative to that destination. `create` and `repack` write a
/// single file at a path the caller named outright — there is no destination
/// directory to hold open, and the name need not even be UTF-8 — so they
/// record the path itself.
#[derive(Debug)]
enum Where {
    /// A path named in full, removed by name.
    Named(PathBuf),
    /// Components under the destination an extraction holds open, removed
    /// through its [`Resolver`].
    Beneath(Vec<String>),
}

/// A place in the ledger, handed back so that one record can be withdrawn.
///
/// It is returned by a record call and accepted by [`Ledger::forget`], which
/// is the only way to withdraw one. Nothing else can name a record, so no
/// caller can withdraw a creation it did not make.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recorded(usize);

/// Every path this run created, in creation order.
#[derive(Debug, Default)]
pub struct Ledger {
    /// The paths this run created, oldest first, each with what it is. The
    /// undo pass walks this in reverse so a directory is removed only after
    /// everything the run put inside it, and it holds only paths this run
    /// created itself: nothing found already in place is ever recorded, and
    /// so nothing found already in place can ever be removed.
    ///
    /// A withdrawn record is left as `None` rather than removed, so that
    /// every [`Recorded`] a caller still holds keeps meaning what it meant.
    created: Vec<Option<(Where, Kind)>>,
}

impl Ledger {
    /// An empty ledger: nothing has been created yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a file, or the marker, that this run created under a
    /// destination it holds open.
    pub fn file(&mut self, components: Vec<String>) -> Recorded {
        self.record(Where::Beneath(components), Kind::File)
    }

    /// Record a directory that this run created under that destination.
    pub fn directory(&mut self, components: Vec<String>) -> Recorded {
        self.record(Where::Beneath(components), Kind::Directory)
    }

    /// Record the one file a single-file writer created at a named path.
    pub fn named_file(&mut self, path: PathBuf) -> Recorded {
        self.record(Where::Named(path), Kind::File)
    }

    /// Append one record and name its place.
    fn record(&mut self, place: Where, kind: Kind) -> Recorded {
        self.created.push(Some((place, kind)));
        Recorded(self.created.len() - 1)
    }

    /// Withdraw a record for a path this run created and has since removed.
    ///
    /// Used for the marker, which is removed on the success path. A path that
    /// no longer exists must not be counted as removed a second time, and must
    /// not be counted as left in place either.
    pub fn forget(&mut self, recorded: Recorded) {
        if let Some(slot) = self.created.get_mut(recorded.0) {
            *slot = None;
        }
    }

    /// Remove everything this run created, newest first, and count the result.
    ///
    /// Reverse order is what makes a directory removable: its children were
    /// created after it. A removal that fails is counted rather than retried,
    /// because the failure this pass is cleaning up after may well be the same
    /// condition — a read-only destination — that stops the removal.
    ///
    /// `resolver` is the same one the run created through, so the undo takes
    /// the arm the creation took; `destination` is used by the portable arm
    /// alone, which is the only one that still joins a path.
    #[must_use]
    pub fn undo(self, resolver: &Resolver, destination: &Path) -> Cleanup {
        let mut cleanup = Cleanup::NONE;
        for (place, kind) in self.created.iter().rev().flatten() {
            let removed = match place {
                Where::Beneath(components) => resolver.remove(destination, components, *kind),
                Where::Named(path) => match kind {
                    Kind::File => std::fs::remove_file(path),
                    Kind::Directory => std::fs::remove_dir(path),
                }
                .map_err(|_| ()),
            };
            if removed.is_ok() {
                cleanup.removed = cleanup.removed.saturating_add(1);
            } else {
                cleanup.left_in_place = cleanup.left_in_place.saturating_add(1);
            }
        }
        cleanup
    }

    /// Undo a ledger that holds named paths alone.
    ///
    /// `create` and `repack` write one file at a path the caller named, and
    /// hold no destination descriptor to resolve anything against. Neither
    /// argument [`Ledger::undo`] takes is consulted for a
    /// [`Where::Named`] record, so this passes the arm that resolves by name
    /// and a destination that is never joined onto anything.
    #[must_use]
    pub fn undo_by_path(self) -> Cleanup {
        self.undo(&Resolver::Portable, Path::new(""))
    }
}
