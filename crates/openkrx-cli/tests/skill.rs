//! The `skill` command: the embedded document, and nothing else.
//!
//! Two things are held here. First, that the command is a pass-through: the
//! bytes on stdout are the file's bytes, with no envelope, no trailing line
//! and nothing on stderr, and that giving it anything to do is a usage error.
//! Second, that the document has not fallen behind the executable — a skill
//! that omits a command or an exit status is worse than no skill, because an
//! agent reads it as the whole surface.
mod support;

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

#[test]
fn the_skill_names_every_command() {
    for command in [
        "capabilities",
        "inspect",
        "list",
        "validate-structure",
        "extract",
        "skill",
    ] {
        assert!(
            SKILL.contains(command),
            "the skill must document the {command} command"
        );
    }
}

#[test]
fn the_skill_names_every_exit_status() {
    // The nine statuses of `Category`, which is not `pub` outside the binary:
    // the list is spelled out so that adding a category without documenting it
    // is a failing test rather than a silent omission. Status 1 is not one of
    // them, and the document says so.
    for status in ["0", "2", "3", "4", "5", "6", "7", "8", "9"] {
        let backticked = format!("`{status}`");
        assert!(
            SKILL.contains(&backticked),
            "the skill must document exit status {status}"
        );
    }
    assert!(SKILL.contains("There is no status `1`"));
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
