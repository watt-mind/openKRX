//! `extract`: what was written, after it was written.
//!
//! Unlike the three reader commands, this report is not a view over a core
//! type: it is the record of what the filesystem layer actually did, built as
//! each file is created. Nothing in it is a prediction. `files_written`,
//! `directories_created` and `bytes_written` count completed operations, and
//! `items[]` holds one entry per file that exists on disk when the command
//! reports success.
//!
//! `items[].path` is the only place in the whole executable where a name from
//! the package reaches the output of a command other than `inspect`, and it
//! appears **only on success**: a failed run reports its stable code, its
//! category and an entry index, and never says which path it was working on.
//! The path is relative to the destination, joined with `/` on every platform,
//! and the destination itself is never reported — the caller named it, and
//! repeating it would put a private path in a report that may be logged.
//!
//! `marker_removed` says the run reached its last step. It is `true` in every
//! successful report; it is a field rather than an assumption so that a
//! consumer can assert it rather than infer it.

use serde::Serialize;

/// What `extract` reports when every planned file was written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExtractData {
    /// Files created, which is every item of the plan.
    pub files_written: u32,
    /// Directories this run created. A directory that already existed is not
    /// counted: this run did not create it.
    pub directories_created: u32,
    /// Bytes written across every file.
    pub bytes_written: u64,
    /// One record per file, in plan order, which is central-directory order.
    pub items: Vec<WrittenView>,
    /// Whether the partial marker was removed, which ends a complete run.
    pub marker_removed: bool,
}

/// One file that exists on disk because this run created it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WrittenView {
    /// Central-directory index of the entry it came from.
    pub entry_index: u32,
    /// The path relative to the destination, components joined with `/`.
    pub path: String,
    /// Bytes written, which is the entry's decoded size.
    pub bytes: u64,
}
