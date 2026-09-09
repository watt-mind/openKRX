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
//! this run created is removed with `remove_dir`, never `remove_dir_all`, so a
//! file someone else put inside it in the meantime keeps the directory alive
//! instead of being deleted with it.
//!
//! The counts the pass produces carry no path: [`Cleanup::removed`] is how
//! many of this run's paths are gone, and [`Cleanup::left_in_place`] how many
//! could not be removed and are still there. A caller reads the second number
//! as "the destination is not as it was found".

use std::path::{Path, PathBuf};

use serde::Serialize;

/// What kind of thing this run created, and therefore how it is removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Created {
    /// A file, or the partial marker; removed with `remove_file`.
    File,
    /// A directory this run created; removed with `remove_dir`, never
    /// recursively, so nothing that is not this run's can go with it.
    Directory,
}

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

/// Every path this run created, in creation order.
#[derive(Debug, Default)]
pub struct Ledger {
    created: Vec<(PathBuf, Created)>,
}

impl Ledger {
    /// An empty ledger: nothing has been created yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a file, or the marker, that this run created.
    pub fn file(&mut self, path: PathBuf) {
        self.created.push((path, Created::File));
    }

    /// Record a directory that this run created.
    pub fn directory(&mut self, path: PathBuf) {
        self.created.push((path, Created::Directory));
    }

    /// Forget a path this run created and has since removed itself.
    ///
    /// Used for the marker, which is removed on the success path. A path that
    /// no longer exists must not be counted as removed a second time, and must
    /// not be counted as left in place either.
    pub fn forget(&mut self, path: &Path) {
        self.created.retain(|(recorded, _)| recorded != path);
    }

    /// Remove everything this run created, newest first, and count the result.
    ///
    /// Reverse order is what makes a directory removable: its children were
    /// created after it. A removal that fails is counted rather than retried,
    /// because the failure this pass is cleaning up after may well be the same
    /// condition — a read-only destination — that stops the removal.
    #[must_use]
    pub fn undo(self) -> Cleanup {
        let mut cleanup = Cleanup::NONE;
        for (path, kind) in self.created.iter().rev() {
            let removed = match kind {
                Created::File => std::fs::remove_file(path),
                Created::Directory => std::fs::remove_dir(path),
            };
            if removed.is_ok() {
                cleanup.removed = cleanup.removed.saturating_add(1);
            } else {
                cleanup.left_in_place = cleanup.left_in_place.saturating_add(1);
            }
        }
        cleanup
    }
}
