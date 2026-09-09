//! The agent skill the binary carries, and the command that writes it out.
//!
//! An AI agent driving openkrx needs the same things a person reads out of
//! `docs/`: which command answers which question, what the envelope and the
//! exit statuses mean, and — above all — what openkrx may not be quoted as
//! saying. `openkrx skill` hands that document over directly, so an agent with
//! nothing but the executable can learn the contract without a checkout.
//!
//! The document is embedded with [`include_str!`], so the copy in the binary
//! and the copy under `crates/openkrx-cli/skills/openkrx/` cannot drift: they
//! are the same bytes, and a build that could not find the file does not
//! compile. A test asserts that the text still names every command and every
//! exit status, so the document cannot fall behind the command surface either.
//!
//! The command is deliberately outside the JSON envelope. It reports on no
//! package, so there is no `command`, no `data` and no `verified` field that
//! would be true of it; wrapping Markdown in a JSON string would only make the
//! one thing the caller wants harder to get at. It takes no file and no
//! `--json` for the same reason, and either one is a usage error.

/// The skill document, embedded at build time.
pub const SKILL: &str = include_str!("../skills/openkrx/SKILL.md");

/// Write the skill to stdout, byte for byte, with nothing added.
///
/// A closed pipe — `openkrx skill | head -5` — is not an error worth a
/// diagnostic, exactly as it is not for any other report, so the write result
/// is dropped and the command still succeeds.
pub fn run() {
    crate::input::payload(&mut std::io::stdout(), SKILL.as_bytes());
}
