//! Editing a package into a new file, under the same output policy as
//! `create`.
//!
//! `openkrx_core::repack` decides *what* the bytes of the edited package are,
//! as a pure function of the package it was given and the edits. This module
//! does the other half, and nothing more: it reads the package and the edits
//! document, resolves the local files the edits name, hands the result to the
//! core crate, and puts the bytes somewhere. It adds no rule about names,
//! ceilings, layout or what may be edited.
//!
//! The run has five phases, in this order:
//!
//! 1. **Read.** The package, then the edits document, then each local file the
//!    edits name, each under the input cap. Nothing about the output has been
//!    touched.
//! 2. **Plan.** `repack::plan` decides what would happen, and refuses a
//!    package it cannot re-emit or an edit the package cannot carry. Nothing
//!    has been written, and nothing has been decided about the destination.
//! 3. **Write the bytes in memory.** The whole package or nothing.
//! 4. **Place.** The `--out` rules are `create`'s, through
//!    [`crate::output`]: an existing parent that is a real directory, a file
//!    that does not exist in any form, `create_new`. `--stdout` places
//!    nothing until phase 5 has passed.
//! 5. **Self-check.** The bytes are read back — from the file that was just
//!    written, when there is one — and put through `archive::inventory`,
//!    `metadata::parse` and `profile::check`. A single failing check is a
//!    defect in openKRX: the file is removed and the run exits 6 with
//!    `repack.internal.self_check_failed`.
//!
//! **The package being edited is never touched.** It is read once, and the
//! result goes to a different file that must not already exist — naming the
//! input as `--out` is refused with `output.exists`, like any other occupied
//! path. openKRX edits no file in place, so an interrupted run can never
//! damage the package someone already had.
//!
//! **What a successful run does not mean.** The result is structurally
//! consistent with the documented layout, which is unverified against every
//! real producer. Nothing was signed, and nothing was verified.

use std::path::{Path, PathBuf};

use openkrx_core::repack::{AttachmentAddition, AttachmentReplacement, Edits, RepackPlan};
use openkrx_core::{Limits, MetadataLimits, archive, create as writer, repack as core};

use crate::commands::repack::RepackData;
use crate::edits::{self, EditsDocument};
use crate::exit::Failure;
use crate::extract::cleanup::{Cleanup, Ledger};
use crate::output::{self, Destination};
use crate::{commands, input};

/// A refused repacking: why it failed, and what was removed again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Refusal {
    /// The failure, carrying its stable code, category and field.
    pub failure: Failure,
    /// How many of this run's paths were removed, and how many were not.
    pub cleanup: Cleanup,
}

/// The package bytes, and the report of reading them back.
pub struct Written {
    /// The report `repack` prints.
    pub data: RepackData,
    /// The bytes themselves, for a destination that has not taken them yet.
    pub bytes: Vec<u8>,
}

/// Edit the package `file` names with the edits in `edits_path`.
///
/// # Errors
///
/// Returns a [`Refusal`]. Its `failure` carries `input.*` when the package,
/// the edits document or a file the edits name could not be read,
/// `manifest.invalid.*` when the edits document does not match the schema,
/// `repack.unsupported.*` when the package carries something the writer cannot
/// re-emit, `repack.invalid.*` when an edit names an attachment the package
/// does not hold, `archive.*` or `metadata.*` when the package could not be
/// read, `create.*` when the writer refused the result, and `output.*` when
/// the destination could not be used.
pub fn run(
    file: &str,
    edits_path: &Path,
    destination: Destination<'_>,
) -> Result<Written, Refusal> {
    let refuse = |failure: Failure| Refusal {
        failure,
        cleanup: Cleanup::NONE,
    };
    // Three different files can fail to open here, and a caller must not have
    // to guess which. The path is never reported — it is the caller's own
    // filesystem — but the *argument* is not content: the edits document is
    // tagged with its own schema root, a file an edit names with the edit that
    // named it, and the package with neither.
    let package = input::read(input::Source::parse(file)).map_err(refuse)?;
    let document = input::read(input::Source::File(edits_path))
        .map_err(|failure| refuse(at_field(failure, edits::field::ROOT)))?;
    let document = edits::parse(&document).map_err(refuse)?;
    let edits = resolve(&document, output::parent_of(edits_path)).map_err(refuse)?;

    let inventory = archive::inventory(&package, &Limits::DEFAULT)
        .map_err(|error| refuse(Failure::from(error)))?;
    let plan = core::plan(
        &inventory,
        &Limits::DEFAULT,
        &MetadataLimits::DEFAULT,
        &edits,
    )
    .map_err(|error| refuse(Failure::from(error)))?;
    let written = core::apply(&inventory, &plan, document.timestamp, &Limits::DEFAULT)
        .map_err(|error| refuse(Failure::from(error)))?;

    let mut ledger = Ledger::new();
    match place(&written, destination, &plan, &mut ledger) {
        Ok(data) => Ok(Written {
            data,
            bytes: written,
        }),
        Err(failure) => Err(Refusal {
            failure,
            cleanup: ledger.undo_by_path(),
        }),
    }
}

/// Put the bytes where they were asked for, then read them back.
///
/// The file is written before the self-check so that what is checked is what a
/// consumer would open, rather than a buffer that was equal to it in memory.
/// `--stdout` has no such file, so its check runs over the bytes about to be
/// written — and runs *before* they are, because standard output cannot be
/// taken back.
fn place(
    package: &[u8],
    destination: Destination<'_>,
    plan: &RepackPlan,
    ledger: &mut Ledger,
) -> Result<RepackData, Failure> {
    let written = match destination {
        Destination::File(path) => {
            output::create_file(package, path, ledger)?;
            input::read(input::Source::File(path))?
        }
        Destination::Stdout => package.to_vec(),
    };
    let report = writer::verify_round_trip(&written, &Limits::DEFAULT, &MetadataLimits::DEFAULT)
        .map_err(|_| Failure::repack_self_check_failed())?;
    if commands::create::failed(&report) {
        return Err(Failure::repack_self_check_failed());
    }
    let length = u64::try_from(written.len()).unwrap_or(u64::MAX);
    Ok(commands::repack::run(length, plan, &report))
}

/// Read every local file the edits name and build the core crate's request.
///
/// A path is joined onto `base` — the edits document's own directory — exactly
/// as written and opened exactly as it then reads: nothing is normalised,
/// matched or globbed, and `..` or an absolute path is the caller's own file,
/// named on the caller's own filesystem. The *package-internal* name never
/// comes from here: `file_name` defaults to the path's last component and goes
/// through the writer's name rules either way.
fn resolve(document: &EditsDocument, base: PathBuf) -> Result<Edits, Failure> {
    let mut add = Vec::with_capacity(document.add.len());
    for (position, addition) in document.add.iter().enumerate() {
        let index = u32::try_from(position).unwrap_or(u32::MAX);
        let bytes = read_at(&base, &addition.path, index, edits::field::ADD_PATH)?;
        add.push(AttachmentAddition {
            file_name: addition
                .file_name
                .clone()
                .unwrap_or_else(|| output::default_file_name(&addition.path)),
            bytes,
            description: addition.description.clone(),
        });
    }
    let mut replace = Vec::with_capacity(document.replace.len());
    for (position, replacement) in document.replace.iter().enumerate() {
        let index = u32::try_from(position).unwrap_or(u32::MAX);
        replace.push(AttachmentReplacement {
            number: replacement.number,
            bytes: read_at(&base, &replacement.path, index, edits::field::REPLACE_PATH)?,
        });
    }
    Ok(Edits {
        header: document.header.clone(),
        remove: document.remove.clone(),
        replace,
        add,
    })
}

/// Read one file the edits name, saying which edit named it.
///
/// `field` is the schema path of the edit — `/add/path` or `/replace/path` —
/// and `index` its position in that array. Neither is content, and together
/// they answer the one question a failed open leaves open: which of the paths
/// on the command line, or in the document, could not be read.
fn read_at(base: &Path, path: &str, index: u32, field: &'static str) -> Result<Vec<u8>, Failure> {
    input::read(input::Source::File(&base.join(path))).map_err(|failure| Failure {
        attachment_index: Some(index),
        ..at_field(failure, field)
    })
}

/// The same failure, scoped to the schema path of the field that named it.
fn at_field(failure: Failure, field: &'static str) -> Failure {
    Failure {
        field: Some(field),
        ..failure
    }
}
