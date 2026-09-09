//! The openKRX reader: argument parsing and dispatch.
//!
//! Three commands read a package and report what is in it: [`inspect`], which
//! prints what the package declares about itself and every structural check;
//! `list`, which prints the archive entries; and `validate-structure`, which
//! prints the check inventory outcome by outcome and encodes the reading in
//! its exit status. Each takes one file, or `-` for standard input, and each
//! accepts `--json`.
//!
//! A fourth command writes: [`extract`] joins the core crate's extraction plan
//! onto a caller-selected destination that must already exist, creates every
//! planned file exclusively, and undoes its own work if anything fails. It
//! never overwrites, never follows a link out of the destination, and never
//! removes anything it did not create.
//!
//! Package semantics live entirely in `openkrx-core`, which performs no I/O.
//! This crate reads the bytes, calls `archive::inventory`, `profile::check`
//! and `extract::plan`, and presents what came back. It adds no rule of its
//! own. Apart from what `extract` was explicitly asked to write, it writes
//! nothing anywhere: no cache is kept and nothing is logged.
//!
//! **Nothing here verifies anything.** openKRX performs no cryptography, so
//! `verified` is `false` in every response and a successful run is not
//! authentication, not proof of delivery and not a legal determination.
//!
//! A fifth command reports on no package at all: [`skill`] writes the agent
//! skill document this binary carries to stdout, so an agent holding nothing
//! but the executable can read the contract it is about to rely on. It is the
//! one command outside the JSON envelope.
//!
//! [`inspect`]: crate::commands::inspect
//! [`extract`]: mod@crate::extract
//! [`skill`]: mod@crate::skill

use clap::{Parser, Subcommand};
use openkrx_core::{Limits, MetadataLimits, archive, capabilities, profile};
use serde::Serialize;

mod commands;
mod exit;
mod extract;
mod input;
mod render;
mod skill;

use exit::{Category, Failure};
use extract::cleanup::Cleanup;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "openkrx",
    version,
    about = "Read a Hungarian KRX document package locally, and extract one into a \
directory you name. Nothing is uploaded and nothing is verified. `skill` \
writes the agent skill this binary carries.",
    after_help = EXIT_STATUS_HELP
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

/// The exit-status table, printed under `openkrx --help`.
///
/// It says which statuses belong to which command, because 3 and 4 belong to
/// `validate-structure` alone: for `inspect` and `list` a failing or undecided
/// check is part of the report, not of the status, and a reader who took the
/// table for a blanket rule would read a broken package as a clean one.
const EXIT_STATUS_HELP: &str = "\
Exit statuses:
  0  the command produced its report; for validate-structure, nothing failed
     and nothing was left undecided
  2  the arguments were rejected
  3  validate-structure only: a structural check failed
  4  validate-structure only: nothing failed, but a rule could not be decided
  5  the input could not be read, or is larger than the 64 MiB input cap
  6  the package is malformed, truncated or ambiguous
  7  the package uses a feature this reader does not implement
  8  a documented parsing limit was exceeded
  9  extract only: the destination could not be used, or a write failed

inspect and list exit 0 whenever they produce their report: read the check
outcomes in the report, or use validate-structure, to act on a failing check.
In --json mode `ok` says only that the command produced a report; read the
exit status, or validate-structure's `summary`, to learn how the checks came
out. Nothing openkrx prints is a conformance verdict and nothing is verified.";

/// `inspect`'s note, so the status split is discoverable from the command's
/// own help rather than only from the top-level table.
const INSPECT_STATUS_HELP: &str = "\
This command exits 0 whenever it produces its report, even when a structural
check failed: the outcome is in the check table it prints. Use
validate-structure to get that reading as an exit status instead.";

/// `extract`'s note. The command writes, so its help says what it will and
/// will not do to the destination before the reader runs it once to find out.
const EXTRACT_STATUS_HELP: &str = "\
The destination directory must already exist and must not be a symbolic link
or a reparse point; it need not be empty. Nothing is ever overwritten: if any
file the package would create is already there, the whole extraction is
refused before anything is written, and exits 9.

While a run is in progress the destination holds .openkrx-extract.partial.
It is removed when the run finishes, so a destination that still contains it
was interrupted: openkrx refuses to extract into it again until you clear it.
If a write fails part-way, every file and directory this run created is
removed again and nothing that was already there is touched.

No permission bits and no timestamps are copied from the package, and no
symbolic link, special file or nested archive is ever created or unpacked.";

/// `list`'s note. It runs no structural check at all, so saying that a failed
/// check is "in the report" would be false here: the report is the entry list.
const LIST_STATUS_HELP: &str = "\
This command runs no structural check and exits 0 whenever it produces its
listing. Use inspect to see the checks, or validate-structure to get their
reading as an exit status.";

#[derive(Subcommand)]
enum Command {
    /// Report implemented operations and development status.
    Capabilities {
        /// Emit one JSON object instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    /// Print what a package declares about itself, and every structural check.
    #[command(after_help = INSPECT_STATUS_HELP)]
    Inspect {
        /// The package to read, or `-` to read standard input.
        #[arg(value_name = "FILE")]
        file: String,
        /// Emit one JSON object instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    /// Print every archive entry, in central-directory order.
    #[command(after_help = LIST_STATUS_HELP)]
    List {
        /// The package to read, or `-` to read standard input.
        #[arg(value_name = "FILE")]
        file: String,
        /// Emit one JSON object instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    /// Write a package's files into a directory that already exists.
    #[command(after_help = EXTRACT_STATUS_HELP)]
    Extract {
        /// The package to read, or `-` to read standard input.
        #[arg(value_name = "FILE")]
        file: String,
        /// The existing directory to write into. It is never created, never
        /// emptied, and nothing in it is ever overwritten.
        #[arg(long, value_name = "DIR")]
        into: PathBuf,
        /// Emit one JSON object instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    /// Report the structural check inventory, outcome by outcome.
    #[command(
        name = "validate-structure",
        after_help = "Exit status: 0 when no check failed and none was left \
undecided, 3 when a check failed, 4 when a rule could not be decided. That is \
a reading of the checks, not a conformance verdict: openkrx cannot make one \
while primary sources leave essential rules open.\n\nIn --json mode read \
`summary`, or the exit status. `ok` reports only that the command produced a \
report, and stays true when a check failed."
    )]
    ValidateStructure {
        /// The package to read, or `-` to read standard input.
        #[arg(value_name = "FILE")]
        file: String,
        /// Emit one JSON object instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    /// Write the embedded agent skill document to standard output.
    #[command(after_help = "The document is written to stdout byte for byte and \
nothing else is: no JSON envelope, no diagnostic, and no file is read. This \
command takes no arguments and no flags, so `skill --json` and `skill FILE` \
are usage errors and exit 2.\n\nSave it where your agent harness looks for \
skills, for example: openkrx skill > .claude/skills/openkrx/SKILL.md")]
    Skill,
}

/// Which reader command is running, and the name its response carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reader {
    Inspect,
    List,
    ValidateStructure,
}

impl Reader {
    /// The stable `command` value of the JSON envelope.
    const fn name(self) -> &'static str {
        match self {
            Self::Inspect => "inspect",
            Self::List => "list",
            Self::ValidateStructure => "validate-structure",
        }
    }
}

fn main() {
    std::process::exit(run());
}

/// Parse the arguments, run the command, and return its exit status.
///
/// Argument errors are taken from the parser rather than left to it, so that
/// every status this executable can exit with comes from [`Category`] and the
/// table in `docs/architecture.md` stays exhaustive. `--help` and `--version`
/// reach the same path: the parser reports them as errors that print on
/// stdout and mean success.
fn run() -> i32 {
    let parsed = match Args::try_parse() {
        Ok(parsed) => parsed,
        Err(error) => {
            let usage = error.use_stderr();
            let _ = error.print();
            return if usage {
                Category::Usage.status()
            } else {
                Category::Success.status()
            };
        }
    };
    // `skill` reports on no package, so it never reaches the envelope, the
    // input reader or a `Category` other than success: it writes the document
    // it carries and stops.
    if matches!(parsed.command, Command::Skill) {
        skill::run();
        return Category::Success.status();
    }
    match parsed.command {
        Command::Skill => unreachable!("skill returned before the dispatch"),
        Command::Capabilities { json } => {
            let data = capabilities();
            let text = if json {
                render::json::success("capabilities", &data)
            } else {
                render::human::capabilities(&data)
            };
            input::line(&mut std::io::stdout(), &text);
            Category::Success.status()
        }
        Command::Inspect { file, json } => reader(Reader::Inspect, &file, json),
        Command::List { file, json } => reader(Reader::List, &file, json),
        Command::ValidateStructure { file, json } => reader(Reader::ValidateStructure, &file, json),
        Command::Extract { file, into, json } => extract(&file, &into, json),
    }
}

/// The name `extract`'s JSON envelope carries.
const EXTRACT: &str = "extract";

/// Run `extract` and report it, in whichever mode was asked for.
///
/// The three phases stay visible here: read the input under the cap, take an
/// inventory of it, and only then let `crate::extract` touch a filesystem.
/// A failure in any of them is reported the same way, so a caller sees one
/// diagnostic shape whether the package could not be read or the destination
/// could not be written.
fn extract(file: &str, into: &std::path::Path, json: bool) -> i32 {
    let bytes = match input::read(input::Source::parse(file)) {
        Ok(bytes) => bytes,
        Err(failure) => return refused(&failure, Cleanup::NONE, json),
    };
    let inventory = match archive::inventory(&bytes, &Limits::DEFAULT) {
        Ok(inventory) => inventory,
        Err(error) => return refused(&Failure::from(error), Cleanup::NONE, json),
    };
    match extract::run(&inventory, into) {
        Ok(data) => {
            let text = if json {
                render::json::success(EXTRACT, &data)
            } else {
                render::human::extract(&data)
            };
            input::line(&mut std::io::stdout(), &text);
            Category::Success.status()
        }
        Err(refusal) => refused(&refusal.failure, refusal.cleanup, json),
    }
}

/// Report a refused extraction and return its exit status.
///
/// Human mode writes two lines on stderr: the failure, and what the undo pass
/// did. JSON mode writes one object on stdout carrying both, and the same two
/// lines on stderr, exactly as every other command does with its diagnostic.
/// Neither mode names the destination or any file.
fn refused(failure: &Failure, cleanup: Cleanup, json: bool) -> i32 {
    if json {
        let text = render::json::failure_with_cleanup(EXTRACT, failure, cleanup);
        input::line(&mut std::io::stdout(), &text);
    }
    input::line(&mut std::io::stderr(), &failure.line());
    input::line(&mut std::io::stderr(), &render::human::cleanup(cleanup));
    failure.category.status()
}

/// Run one reader command and report it, in whichever mode was asked for.
fn reader(command: Reader, file: &str, json: bool) -> i32 {
    match read(command, file, json) {
        Ok((text, category)) => {
            input::line(&mut std::io::stdout(), &text);
            category.status()
        }
        Err(failure) => {
            if json {
                let text = render::json::failure(command.name(), &failure);
                input::line(&mut std::io::stdout(), &text);
            }
            input::line(&mut std::io::stderr(), &failure.line());
            failure.category.status()
        }
    }
}

/// Read the input and build the report, without touching a stream.
///
/// # Errors
///
/// Returns the first [`Failure`] the input, the archive reader or the profile
/// layer produced. A structural check that did not hold is not a failure here:
/// it is part of the report, and only `validate-structure` turns it into an
/// exit status.
fn read(command: Reader, file: &str, json: bool) -> Result<(String, Category), Failure> {
    let bytes = input::read(input::Source::parse(file))?;
    let inventory = archive::inventory(&bytes, &Limits::DEFAULT)?;
    match command {
        Reader::List => {
            let data = commands::list::run(&inventory);
            Ok((
                present(json, command, &data, || render::human::list(&data)),
                Category::Success,
            ))
        }
        Reader::Inspect => {
            let report = profile::check(&inventory, &MetadataLimits::DEFAULT)?;
            let data = commands::inspect::run(&inventory, &report);
            Ok((
                present(json, command, &data, || render::human::inspect(&data)),
                Category::Success,
            ))
        }
        Reader::ValidateStructure => {
            let report = profile::check(&inventory, &MetadataLimits::DEFAULT)?;
            let (data, category) = commands::validate::run(&report);
            Ok((
                present(json, command, &data, || render::human::validate(&data)),
                category,
            ))
        }
    }
}

/// One report, as a JSON object or as text.
fn present<T: Serialize>(
    json: bool,
    command: Reader,
    data: &T,
    human: impl FnOnce() -> String,
) -> String {
    if json {
        render::json::success(command.name(), data)
    } else {
        human()
    }
}
