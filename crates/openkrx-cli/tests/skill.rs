//! The `skill` command: the embedded document, and nothing else.
//!
//! Two things are held here. First, that the command is a pass-through: the
//! bytes on stdout are the file's bytes, with no envelope, no trailing line
//! and nothing on stderr, and that giving it anything to do is a usage error.
//! Second, that the document has not fallen behind the executable — a skill
//! that omits a command or an exit status is worse than no skill, because an
//! agent reads it as the whole surface.
mod support;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use support::{run, status, stderr, stdout};

/// The same bytes the binary embedded, read here by the same mechanism, so a
/// test that passed could not have compared the output against itself.
const SKILL: &str = include_str!("../skills/openkrx/SKILL.md");

#[test]
fn skill_writes_the_embedded_document_and_nothing_else() {
    let output = run(&["skill"]);
    assert_eq!(status(&output), 0);
    assert!(output.stderr.is_empty(), "stderr must stay empty");
    assert_eq!(
        output.stdout,
        SKILL.as_bytes(),
        "stdout must be the embedded document byte for byte"
    );
}

#[test]
fn skill_writes_no_envelope() {
    // A caller redirecting the output into a skills directory must get
    // Markdown, not a JSON object carrying Markdown in a string.
    let text = stdout(&run(&["skill"]));
    // The document quotes the envelope, so its absence is asserted on the
    // output's shape rather than on a substring: Markdown front matter first,
    // and no JSON object anywhere on the first or the last line.
    assert!(text.starts_with("---\nname: openkrx\n"));
    let first = text.lines().next().expect("a first line");
    let last = text.lines().next_back().expect("a last line");
    assert!(!first.starts_with('{'), "no envelope opens the output");
    assert!(!last.ends_with('}'), "no envelope closes the output");
    assert!(serde_json::from_str::<serde_json::Value>(&text).is_err());
}

#[test]
fn skill_rejects_the_json_flag() {
    let output = run(&["skill", "--json"]);
    assert_eq!(status(&output), 2);
    assert!(output.stdout.is_empty(), "a usage error writes no report");
    assert!(!stderr(&output).is_empty());
}

#[test]
fn skill_rejects_a_file_argument() {
    let output = run(&["skill", "package.krx"]);
    assert_eq!(status(&output), 2);
    assert!(output.stdout.is_empty(), "a usage error writes no report");
    assert!(!stderr(&output).is_empty());
}

/// The subcommand names clap prints under `Commands:` in the top-level help.
///
/// The binary is the source: a subcommand added to the parser appears here
/// without anything being edited, so the drift test below fails until the
/// document names it. `help` is clap's own built-in and not part of the
/// surface openkrx publishes, so it is dropped.
///
/// Only lines indented exactly two spaces are entries; clap indents the
/// continuation of a wrapped description further, and those must not be read
/// as command names.
fn commands_from_help(help: &str) -> Vec<String> {
    section_lines(help, "Commands:")
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| *name != "help")
        .map(str::to_owned)
        .collect()
}

/// The digits of the exit-status table printed under `openkrx --help`.
///
/// That table is `EXIT_STATUS_HELP` in the binary, which is the same list
/// `Category::status` produces; the crate is a binary, so the enum cannot be
/// named from an integration test and the printed table is the closest source
/// of truth reachable without changing `src/`.
fn exit_statuses_from_help(help: &str) -> Vec<String> {
    section_lines(help, "Exit statuses:")
        .filter_map(|line| line.split_whitespace().next())
        .filter(|token| token.chars().all(|c| c.is_ascii_digit()))
        .map(str::to_owned)
        .collect()
}

/// The lines of a help section: everything indented exactly two spaces between
/// `heading` and the blank line that closes the section.
fn section_lines<'a>(help: &'a str, heading: &'a str) -> impl Iterator<Item = &'a str> {
    help.lines()
        .skip_while(move |line| !line.starts_with(heading))
        .skip(1)
        .take_while(|line| !line.trim().is_empty())
        .filter(|line| line.starts_with("  ") && !line.starts_with("   "))
        .map(str::trim)
}

#[test]
fn the_skill_names_every_command() {
    let help = stdout(&run(&["--help"]));
    let commands = commands_from_help(&help);
    assert!(
        commands.len() > 1,
        "the Commands section of the help did not parse: {commands:?}"
    );
    for command in commands {
        assert!(
            SKILL.contains(&command),
            "the skill must document the {command} command"
        );
    }
}

#[test]
fn the_skill_names_every_exit_status() {
    // Derived from the table the binary prints, so a new category — which must
    // be added to `EXIT_STATUS_HELP` to be documented at all — fails this test
    // until the skill names it too. Status 1 is not one of them, and the
    // document says so.
    let help = stdout(&run(&["--help"]));
    let statuses = exit_statuses_from_help(&help);
    assert!(
        statuses.len() > 1,
        "the exit-status table of the help did not parse: {statuses:?}"
    );
    assert!(
        !statuses.iter().any(|status| status == "1"),
        "status 1 is not a category the CLI uses"
    );
    for status in statuses {
        let backticked = format!("`{status}`");
        assert!(
            SKILL.contains(&backticked),
            "the skill must document exit status {status}"
        );
    }
    assert!(SKILL.contains("There is no status `1`"));
}

/// JSON keys the goldens carry that the skill deliberately does not name.
///
/// Each entry needs a one-line reason. A key belongs here only when an agent
/// has no use for it; a key an agent would read belongs in SKILL.md instead.
const UNDOCUMENTED_JSON_KEYS: &[(&str, &str)] = &[
    // Empty: every key the --json goldens carry is named in the document.
];

/// The `tests/golden` directory, from this crate's manifest.
fn golden_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden")
}

/// Every object key appearing anywhere in `value`, at any depth.
fn json_keys(value: &serde_json::Value, into: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                into.insert(key.clone());
                json_keys(child, into);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                json_keys(item, into);
            }
        }
        _ => {}
    }
}

/// Every identifier the document writes inside backticks, one path segment at
/// a time, so that `data.entries[]` documents `data` and `entries` both.
fn documented_identifiers(skill: &str) -> BTreeSet<String> {
    skill
        .split('`')
        .skip(1)
        .step_by(2)
        .flat_map(|span| span.split(|c: char| !c.is_ascii_alphanumeric() && c != '_'))
        .filter(|token| !token.is_empty())
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_skill_names_every_json_field_the_goldens_carry() {
    // SKILL.md tells an agent which fields to read. The golden `--json`
    // outputs are the recorded shape of every response, so a renamed field
    // fails here rather than leaving the document quietly wrong.
    let mut keys = BTreeSet::new();
    let mut files = 0usize;
    for entry in std::fs::read_dir(golden_root()).expect("read the golden directory") {
        let path = entry.expect("a golden directory entry").path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if !name.ends_with("-json") {
            continue;
        }
        let stdout_path = path.join("stdout");
        let text = std::fs::read_to_string(&stdout_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", stdout_path.display()));
        let value: serde_json::Value = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("parse {}: {e}", stdout_path.display()));
        json_keys(&value, &mut keys);
        files += 1;
    }
    assert!(files > 1, "no --json goldens were found under tests/golden");
    assert!(!keys.is_empty(), "the --json goldens carried no keys");

    let documented = documented_identifiers(SKILL);
    for (key, reason) in UNDOCUMENTED_JSON_KEYS {
        assert!(
            keys.contains(*key),
            "`{key}` is allow-listed ({reason}) but no golden carries it"
        );
    }
    for key in &keys {
        if UNDOCUMENTED_JSON_KEYS.iter().any(|(k, _)| k == key) {
            continue;
        }
        assert!(
            documented.contains(key),
            "the skill must name the `{key}` JSON field, or allow-list it with a reason"
        );
    }
}

#[test]
fn the_skill_states_the_boundary_it_exists_to_carry() {
    // The reason for shipping a skill at all: an agent that reads it must not
    // be able to come away thinking openkrx verifies or validates anything.
    assert!(SKILL.contains("not signature verification"));
    assert!(SKILL.contains("`verified` is `false` in every response"));
    assert!(SKILL.contains("Never claim conformance, validity or authenticity"));
    for rule in [
        "A19", "A20", "A21", "A22", "M11", "M12", "M13", "M14", "M15",
    ] {
        assert!(SKILL.contains(rule), "the skill must name rule {rule}");
    }
}

#[test]
fn capabilities_still_lists_only_the_package_operations() {
    // `skill` operates on no package, so it is not a capability: a consumer
    // branching on `operations` must not be told openkrx grew one.
    let text = stdout(&run(&["capabilities", "--json"]));
    assert!(text.contains(r#""operations":["inspect","list","validate-structure","extract"]"#));
    assert!(!text.contains(r#""skill""#));
}

#[test]
fn the_help_lists_the_skill_command() {
    let text = stdout(&run(&["--help"]));
    assert!(text.contains("skill"));
}
