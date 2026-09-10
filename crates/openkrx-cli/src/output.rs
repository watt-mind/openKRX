//! Where a written package goes, and the rules that protect what is there.
//!
//! `create` and `repack` both produce the bytes of one package and then have
//! to put them somewhere. That second half is the same for both, and it is
//! here so that it cannot drift apart: the parent directory must already exist
//! and be real, the file must not exist in any form, and it is created
//! exclusively. Nothing is ever overwritten, and a failure after the file was
//! created removes it again through the caller's [`Ledger`].
//!
//! The rules are `extract`'s, for the reason `extract` has them: a typo in an
//! argument must produce a refusal rather than a new tree, and a run that did
//! not report success must never leave a half-written package behind for
//! someone to send.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::exit::{
    Failure, OUTPUT_DESTINATION_MISSING, OUTPUT_DESTINATION_NOT_A_DIRECTORY,
    OUTPUT_DESTINATION_SYMLINK, OUTPUT_EXISTS, OUTPUT_IO,
};
use crate::extract::cleanup::Ledger;
use crate::extract::preflight::is_link;

/// Where the package bytes go.
#[derive(Debug, Clone, Copy)]
pub enum Destination<'a> {
    /// A file that must not exist yet, created exclusively.
    File(&'a Path),
    /// Standard output, written only after the self-check has passed.
    Stdout,
}

/// Create the output file exclusively, after checking what is already there.
///
/// The parent must already exist as a real directory: neither command creates
/// one, for the reason `extract` never creates its destination — a typo in the
/// argument would silently produce a tree instead of a refusal. The file
/// itself must not exist in any form, which `symlink_metadata` answers without
/// following a link, so a dangling symbolic link counts as occupied rather
/// than as free space. `create_new` then closes the window between that check
/// and the creation.
///
/// # Errors
///
/// An `output.*` [`Failure`]: the parent is missing, is not a directory or is
/// a link; something is already at the path; or the write itself failed.
pub fn create_file(package: &[u8], path: &Path, ledger: &mut Ledger) -> Result<(), Failure> {
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

/// The directory a relative path inside a document is resolved against.
///
/// The document's own directory, so that a manifest or an edits file and the
/// files it names move together, and the working directory the command
/// happened to run in changes nothing about the package that comes out.
pub fn parent_of(document: &Path) -> PathBuf {
    match document.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

/// The last component of a path, as a package-internal name.
///
/// A path with no last component — one ending in a separator or in `..` —
/// yields an empty name, which the writer refuses with
/// `create.unsafe_name.empty`: guessing a name for a file whose own name
/// openKRX could not read would be an invention.
pub fn default_file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_owned()
}
