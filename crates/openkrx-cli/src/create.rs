//! Turning a manifest into a package file, under the same output policy as
//! `extract`.
//!
//! `openkrx_core::create::package` decides *what* the bytes of a package are,
//! as a pure function of a `PackageSpec`. This module does the other half, and
//! nothing more: it resolves the manifest's attachment paths, reads those
//! files under the input cap, hands the result to the writer, and puts the
//! bytes somewhere. It adds no rule about names, ceilings or layout — those
//! belong to the writer, which refuses before anything is opened for writing.
//!
//! The run has four phases, in this order:
//!
//! 1. **Read.** The manifest, then each attachment, each under the input cap.
//!    Nothing about the output has been touched.
//! 2. **Write the bytes in memory.** The whole package or nothing: a refusal
//!    refuses the package rather than writing a reduced one.
//! 3. **Place.** For `--out`: the parent must be a real directory, the file
//!    must not exist in any form, and it is created with `create_new`. For
//!    `--stdout`: nothing is placed until phase 4 has passed.
//! 4. **Self-check.** The bytes are read back — from the file that was just
//!    written, when there is one — and put through `archive::inventory`,
//!    `metadata::parse` and `profile::check`. A single failing check is a
//!    defect in openKRX: the file is removed and the run exits 6 with
//!    `create.internal.self_check_failed`.
//!
//! **Failure policy.** Nothing is ever overwritten, and a failure after the
//! file was created removes it again: a `create` that did not report success
//! never leaves a half-written package behind for someone to send. The counts
//! are reported exactly as `extract` reports them, as a `Cleanup`.
//!
//! **What a successful run does not mean.** The package is structurally
//! consistent with the documented layout, which is unverified against every
//! real producer. `validate-structure` over it exits `4`, citing the rules
//! `docs/profile.md` leaves open. Nothing was signed, and nothing was
//! verified.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use openkrx_core::create::{AttachmentInput, PackageSpec};
use openkrx_core::{Limits, MetadataLimits, create as writer};

use crate::commands::create::CreateData;
use crate::exit::{
    Failure, OUTPUT_DESTINATION_MISSING, OUTPUT_DESTINATION_NOT_A_DIRECTORY,
    OUTPUT_DESTINATION_SYMLINK, OUTPUT_EXISTS, OUTPUT_IO,
};
use crate::extract::cleanup::{Cleanup, Ledger};
use crate::extract::preflight::is_link;
use crate::manifest::{self, Manifest};
use crate::{commands, input};

/// Where the package bytes go.
#[derive(Debug, Clone, Copy)]
pub enum Destination<'a> {
    /// A file that must not exist yet, created exclusively.
    File(&'a Path),
    /// Standard output, written only after the self-check has passed.
    Stdout,
}

/// A refused creation: why it failed, and what was removed again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Refusal {
    /// The failure, carrying its stable code, category and manifest field.
    pub failure: Failure,
    /// How many of this run's paths were removed, and how many were not.
    pub cleanup: Cleanup,
}

/// The package bytes, and the report of reading them back.
pub struct Written {
    /// The report `create` prints.
    pub data: CreateData,
    /// The bytes themselves, for a destination that has not taken them yet.
    pub bytes: Vec<u8>,
}

/// Write the package `manifest_path` describes to `destination`.
///
/// # Errors
///
/// Returns a [`Refusal`]. Its `failure` carries `input.*` when the manifest or
/// an attachment could not be read, `manifest.invalid.*` when the manifest
/// does not describe a package this build can write, `create.*` when the
/// writer refused the request or its own output, and `output.*` when the
/// destination could not be used. Its `cleanup` says whether a partly written
/// file was removed again.
pub fn run(manifest_path: &Path, destination: Destination<'_>) -> Result<Written, Refusal> {
    let refuse = |failure: Failure| Refusal {
        failure,
        cleanup: Cleanup::NONE,
    };
    let bytes = input::read(input::Source::File(manifest_path)).map_err(refuse)?;
    let manifest = manifest::parse(&bytes).map_err(refuse)?;
    let spec = spec(&manifest, parent_of(manifest_path)).map_err(refuse)?;
    let package =
        writer::package(&spec, &Limits::DEFAULT).map_err(|error| refuse(Failure::from(error)))?;
    let entries = u32::try_from(spec.attachments.len().saturating_add(2)).unwrap_or(u32::MAX);

    let mut ledger = Ledger::new();
    match place(&package, destination, entries, &mut ledger) {
        Ok(data) => Ok(Written {
            data,
            bytes: package,
        }),
        Err(failure) => Err(Refusal {
            failure,
            cleanup: ledger.undo(),
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
    entries: u32,
    ledger: &mut Ledger,
) -> Result<CreateData, Failure> {
    let written = match destination {
        Destination::File(path) => {
            create_file(package, path, ledger)?;
            input::read(input::Source::File(path))?
        }
        Destination::Stdout => package.to_vec(),
    };
    let report = writer::verify_round_trip(&written, &Limits::DEFAULT, &MetadataLimits::DEFAULT)
        .map_err(|_| Failure::self_check_failed())?;
    if commands::create::failed(&report) {
        return Err(Failure::self_check_failed());
    }
    let length = u64::try_from(written.len()).unwrap_or(u64::MAX);
    Ok(commands::create::run(length, entries, &report))
}

/// Create the output file exclusively, after checking what is already there.
///
/// The parent must already exist as a real directory: `create` never creates
/// one, for the reason `extract` never creates its destination — a typo in the
/// argument would silently produce a tree instead of a refusal. The file
/// itself must not exist in any form, which `symlink_metadata` answers without
/// following a link, so a dangling symbolic link counts as occupied rather
/// than as free space. `create_new` then closes the window between that check
/// and the creation.
fn create_file(package: &[u8], path: &Path, ledger: &mut Ledger) -> Result<(), Failure> {
    parent(path)?;
    match std::fs::symlink_metadata(path) {
        Ok(_) => return Err(Failure::output(OUTPUT_EXISTS)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(Failure::output(OUTPUT_IO)),
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                Failure::output(OUTPUT_EXISTS)
            } else {
                Failure::output(OUTPUT_IO)
            }
        })?;
    ledger.file(path.to_path_buf());
    file.write_all(package)
        .map_err(|_| Failure::output(OUTPUT_IO))?;
    file.flush().map_err(|_| Failure::output(OUTPUT_IO))?;
    Ok(())
}

/// The directory the output file goes in: it must be there, and be real.
fn parent(path: &Path) -> Result<(), Failure> {
    let directory = match path.parent() {
        // An empty parent is what `--out package.krx` produces: the file goes
        // in the working directory, which the process is already in.
        Some(parent) if parent.as_os_str().is_empty() => return Ok(()),
        Some(parent) => parent,
        None => return Err(Failure::output(OUTPUT_DESTINATION_NOT_A_DIRECTORY)),
    };
    let metadata = std::fs::symlink_metadata(directory).map_err(|error| {
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

/// The directory a relative attachment path is resolved against.
///
/// The manifest's own directory, so that a manifest and its attachments move
/// together and the working directory the command happened to run in changes
/// nothing about the package that comes out.
fn parent_of(manifest_path: &Path) -> PathBuf {
    match manifest_path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

/// Read every attachment and build the request the writer takes.
///
/// A path is joined onto `base` exactly as written and opened exactly as it
/// then reads: nothing is normalised, matched or globbed, and `..` or an
/// absolute path is the caller's own file, named on the caller's own
/// filesystem. The *package-internal* name is the other half, and it never
/// comes from here: `file_name` defaults to the path's last component and goes
/// through the writer's name rules, which refuse a separator, a parent
/// component, a control character and a Windows device name.
fn spec(manifest: &Manifest, base: PathBuf) -> Result<PackageSpec, Failure> {
    let mut attachments = Vec::with_capacity(manifest.attachments.len());
    for (position, attachment) in manifest.attachments.iter().enumerate() {
        let index = u32::try_from(position).unwrap_or(u32::MAX);
        let path = base.join(&attachment.path);
        let bytes = input::read(input::Source::File(&path)).map_err(|failure| Failure {
            attachment_index: Some(index),
            ..failure
        })?;
        let file_name = attachment
            .file_name
            .clone()
            .unwrap_or_else(|| default_file_name(&attachment.path));
        attachments.push(AttachmentInput {
            file_name,
            bytes,
            description: attachment.description.clone(),
            quantity: None,
            quantity_unit: None,
        });
    }
    Ok(PackageSpec {
        attachments,
        timestamp: manifest.timestamp,
        ..PackageSpec::new(manifest.metadata.clone())
    })
}

/// The last component of a manifest path, as the package-internal name.
///
/// A path with no last component — one ending in a separator or in `..` —
/// yields an empty name, which the writer refuses with
/// `create.unsafe_name.empty`: guessing a name for a file whose own name
/// openKRX could not read would be an invention.
fn default_file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_owned()
}
