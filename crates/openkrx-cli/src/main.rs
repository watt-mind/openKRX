//! The openKRX reader: argument parsing and dispatch.
//!
//! Three commands read a package and report what is in it: [`inspect`], which
//! prints what the package declares about itself and every structural check;
//! `list`, which prints the archive entries; and `validate-structure`, which
//! prints the check inventory outcome by outcome and encodes the reading in
//! its exit status. Each takes one file, or `-` for standard input, and each
//! accepts `--json`.
//!
//! Package semantics live entirely in `openkrx-core`, which performs no I/O.
//! This crate reads the bytes, calls `archive::inventory` and `profile::check`,
//! and presents what came back. It adds no rule of its own, and it writes
//! nothing anywhere: no file is created, no cache is kept and nothing is
//! logged.
//!
//! **Nothing here verifies anything.** openKRX performs no cryptography, so
//! `verified` is `false` in every response and a successful run is not
//! authentication, not proof of delivery and not a legal determination.
//!
//! [`inspect`]: crate::commands::inspect

use clap::{Parser, Subcommand};
use openkrx_core::{Limits, MetadataLimits, archive, capabilities, profile};
use serde::Serialize;

mod commands;
mod exit;
mod input;
mod render;

use exit::{Category, Failure};

#[derive(Parser)]
#[command(
    name = "openkrx",
    version,
    about = "Read a Hungarian KRX document package locally. Nothing is uploaded, \
nothing is written and nothing is verified.",
    after_help = "Exit statuses: 0 success, 2 usage, 3 a structural check failed, \
4 a rule could not be decided, 5 the input could not be read, 6 malformed package, \
7 unsupported feature, 8 resource limit exceeded."
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Report implemented operations and development status.
    Capabilities {
        /// Emit one JSON object instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    /// Print what a package declares about itself, and every structural check.
    Inspect {
        /// The package to read, or `-` to read standard input.
        #[arg(value_name = "FILE")]
        file: String,
        /// Emit one JSON object instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    /// Print every archive entry, in central-directory order.
    List {
        /// The package to read, or `-` to read standard input.
        #[arg(value_name = "FILE")]
        file: String,
        /// Emit one JSON object instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    /// Report the structural check inventory, outcome by outcome.
    #[command(name = "validate-structure")]
    ValidateStructure {
        /// The package to read, or `-` to read standard input.
        #[arg(value_name = "FILE")]
        file: String,
        /// Emit one JSON object instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
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
    match parsed.command {
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
    }
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
            input::line(
                &mut std::io::stderr(),
                &format!("openkrx: {}", failure.message()),
            );
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
