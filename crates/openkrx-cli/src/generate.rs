//! The two documents the parser can write about itself: a shell completion
//! script, and a manual page.
//!
//! Both are generated from [`crate::Args::command()`] — the same `clap`
//! definition the binary dispatches on — at run time rather than at build
//! time. Nothing is committed, nothing is copied, and there is no second
//! description of the command surface that could fall behind the first: a
//! subcommand added to the parser appears in the completion script and in the
//! man page of the very same executable, without anything being edited.
//!
//! Both commands sit outside the JSON envelope, exactly as [`crate::skill`]
//! does and for the same reason. They report on no package, so there is no
//! `command`, no `data` and no `verified` field that would be true of them,
//! and a caller redirecting the output into a completion directory or into
//! `man1/` wants the document, not a JSON string carrying it. Neither takes a
//! file and neither takes `--json`; either is a usage error and exits 2.
//!
//! The bytes depend on the `clap_complete` and `clap_mangen` versions this
//! binary was built against, so neither has a golden case: the contract held
//! here is that the script and the page are generated from the live parser,
//! not that they are any particular sequence of bytes.

use clap::CommandFactory;
use clap_complete::Shell;

use crate::Args;

/// The name the generated documents call this program, which is also the
/// `[[bin]]` name and the word a completion script hooks onto.
const PROGRAM: &str = "openkrx";

/// Write the completion script for `shell` to stdout.
///
/// `clap_complete` writes the script itself, so a closed pipe — `openkrx
/// completions bash | head -3` — is swallowed by the writer below rather than
/// turned into a diagnostic, exactly as it is for every other report.
pub fn completions(shell: Shell) {
    let mut command = Args::command();
    let mut out = Sink(std::io::stdout());
    clap_complete::generate(shell, &mut command, PROGRAM, &mut out);
}

/// Write the roff manual page to stdout.
///
/// One page is written, for the binary as a whole: `clap_mangen` renders the
/// subcommands into a section of it, so `man openkrx` documents the whole
/// surface and there is no set of per-subcommand pages to install, name or
/// keep in step.
pub fn man() {
    let command = Args::command();
    let mut page = Vec::new();
    // Rendering into memory cannot fail — `Vec` never returns an I/O error —
    // and a failure to write the finished page to a closed pipe is not worth a
    // diagnostic, so neither result reaches the caller.
    if clap_mangen::Man::new(command).render(&mut page).is_ok() {
        crate::input::payload(&mut std::io::stdout(), &page);
    }
}

/// A writer that drops I/O errors, so a closed pipe is not an error.
///
/// `clap_complete::generate` takes a [`std::io::Write`] and panics on a write
/// error rather than reporting it, which would turn `openkrx completions bash
/// | head -3` into a panic message. Reporting success for every write is the
/// same decision [`crate::input::payload`] makes, in the shape that API needs.
struct Sink(std::io::Stdout);

impl std::io::Write for Sink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let _ = std::io::Write::write_all(&mut self.0, buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let _ = std::io::Write::flush(&mut self.0);
        Ok(())
    }
}
