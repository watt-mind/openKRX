//! Bounded reading of the one input a reader command takes.
//!
//! The core crate performs no I/O, so this is the only place in openKRX that
//! opens anything. Reading is bounded before parsing begins: at most
//! [`INPUT_CAP_BYTES`] bytes are ever buffered, which is one byte past
//! `Limits::DEFAULT.max_archive_bytes`. An input that reaches the cap is
//! refused with `input.over_limit.archive_bytes` without the rest of it ever
//! being read, so a very large file cannot be turned into memory pressure by
//! naming it on the command line.
//!
//! The file argument is opened exactly as written. No path normalisation,
//! globbing, symlink resolution or extension inference happens, on any
//! platform, and the path never appears in a diagnostic.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use crate::exit::{Failure, INPUT_CAP_BYTES};

/// Where the bytes come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source<'a> {
    /// Standard input, requested as the argument `-`, read as binary.
    Stdin,
    /// A file, opened exactly as the argument spells it.
    File(&'a Path),
}

impl<'a> Source<'a> {
    /// Read the argument as `-` for standard input, or as a path.
    #[must_use]
    pub fn parse(argument: &'a str) -> Self {
        if argument == "-" {
            Self::Stdin
        } else {
            Self::File(Path::new(argument))
        }
    }
}

/// Read the whole input, refusing anything past the cap.
///
/// # Errors
///
/// Returns `input.unreadable` when the source cannot be opened or read — a
/// missing file, a directory, a permission failure or a broken pipe are all
/// the same content-free code — and `input.over_limit.archive_bytes` when the
/// input reaches the cap. Neither carries the path.
pub fn read(source: Source<'_>) -> Result<Vec<u8>, Failure> {
    match source {
        Source::Stdin => {
            let stdin = std::io::stdin();
            let mut handle = stdin.lock();
            read_capped(&mut handle)
        }
        Source::File(path) => {
            let mut file = File::open(path).map_err(|_| Failure::unreadable())?;
            read_capped(&mut file)
        }
    }
}

/// Buffer at most one byte past the archive ceiling, then decide.
fn read_capped(reader: &mut impl Read) -> Result<Vec<u8>, Failure> {
    let cap = usize::try_from(INPUT_CAP_BYTES).map_err(|_| Failure::unreadable())?;
    let mut bytes = Vec::new();
    reader
        .take(INPUT_CAP_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|_| Failure::unreadable())?;
    if bytes.len() >= cap {
        return Err(Failure::over_input_cap());
    }
    Ok(bytes)
}

/// Write one line to a stream, ignoring a closed pipe.
///
/// A consumer that closes the pipe early — `openkrx list … | head` — is not an
/// error worth a diagnostic, and a diagnostic about a broken stderr could not
/// be delivered anyway.
pub fn line(stream: &mut impl Write, text: &str) {
    let _ = writeln!(stream, "{text}");
}

/// Write bytes to a stream exactly as they are, adding nothing.
///
/// [`line()`] terminates what it writes, which is right for a report and wrong
/// for a document that already ends as its author wrote it. A closed pipe is
/// ignored here for the same reason it is there.
pub fn payload(stream: &mut impl Write, bytes: &[u8]) {
    let _ = stream.write_all(bytes);
}
