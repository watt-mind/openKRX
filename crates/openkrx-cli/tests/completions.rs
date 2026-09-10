//! The `completions` and `man` commands: two documents generated from the
//! parser, and nothing else.
//!
//! Both exist so that a shell and `man` can describe the binary a user
//! actually has. What is held here is therefore not any particular byte —
//! those belong to the `clap_complete` and `clap_mangen` versions this build
//! links against, which is why neither command has a golden case — but that
//! the documents are generated from the live parser: every subcommand the
//! executable's own `--help` lists must appear in every completion script and
//! in the man page, and the man page must carry the whole exit-status table.
//! A subcommand added to `Args` therefore passes without this file being
//! touched, and a document generated from a stale second description of the
//! surface would fail.
//!
//! The pass-through contract is held too, exactly as it is for `skill`: no
//! envelope, nothing on stderr, and anything else on the command line is a
//! usage error that exits 2 with an empty stdout.
mod support;

use support::{run, status, stderr, stdout};

/// The five shells `clap_complete` supports, spelled as the argument takes
/// them. A shell added to `clap_complete::Shell` is not automatically part of
/// openkrx's contract, so this list is deliberately written out.
const SHELLS: [&str; 5] = ["bash", "zsh", "fish", "powershell", "elvish"];

/// The subcommand names clap prints under `Commands:` in the top-level help.
///
/// The binary is the source, exactly as it is in `skill.rs`: a subcommand
/// added to the parser appears here without anything being edited, and the
/// tests below then require the generated documents to name it. `help` is
/// clap's own built-in and not part of the surface openkrx publishes, so it is
/// dropped.
fn commands_from_help(help: &str) -> Vec<String> {
    section_lines(help, "Commands:")
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| *name != "help")
        .map(str::to_owned)
        .collect()
}

/// The digits of the exit-status table printed under `openkrx --help`.
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

/// The man page with roff's escapes undone far enough to search it as text.
///
/// `clap_mangen` escapes a hyphen as `\-` and a leading dot, so
/// `validate-structure` is written `validate\-structure` in the page and a
/// plain substring search for the command name would fail on a page that does
/// document it.
fn as_text(page: &str) -> String {
    page.replace("\\-", "-").replace("\\&", "")
}

#[test]
fn every_shell_gets_a_script_naming_every_command() {
    let commands = commands_from_help(&stdout(&run(&["--help"])));
    assert!(
        commands.len() >= 8,
        "the Commands section of the help did not parse: {commands:?}"
    );
    for shell in SHELLS {
        let output = run(&["completions", shell]);
        assert_eq!(status(&output), 0, "{shell} completions must succeed");
        assert!(
            output.stderr.is_empty(),
            "{shell} completions must write nothing on stderr"
        );
        let script = stdout(&output);
        assert!(
            !script.trim().is_empty(),
            "{shell} completions must write a script"
        );
        assert!(
            script.contains("openkrx"),
            "the {shell} script must hook onto the program name"
        );
        for command in &commands {
            assert!(
                script.contains(command.as_str()),
                "the {shell} script must complete the {command} command"
            );
        }
    }
}

#[test]
fn completions_writes_no_envelope() {
    // A caller redirecting the output into a completion directory must get a
    // shell script, not a JSON object carrying one.
    let script = stdout(&run(&["completions", "bash"]));
    assert!(serde_json::from_str::<serde_json::Value>(&script).is_err());
    let first = script.lines().next().expect("a first line");
    assert!(!first.starts_with('{'), "no envelope opens the output");
}

#[test]
fn the_man_page_names_every_command_and_every_exit_status() {
    let help = stdout(&run(&["--help"]));
    let commands = commands_from_help(&help);
    let statuses = exit_statuses_from_help(&help);
    assert!(
        commands.len() >= 8,
        "the Commands section of the help did not parse: {commands:?}"
    );
    assert!(
        statuses.len() >= 9,
        "the exit-status table of the help did not parse: {statuses:?}"
    );

    let output = run(&["man"]);
    assert_eq!(status(&output), 0);
    assert!(output.stderr.is_empty(), "stderr must stay empty");
    let page = stdout(&output);
    assert!(page.starts_with(".ie"), "the page must open as roff");
    assert!(
        page.contains(".SH NAME"),
        "the page must carry a NAME section"
    );
    let text = as_text(&page);
    for command in &commands {
        assert!(
            text.contains(command.as_str()),
            "the man page must document the {command} command"
        );
    }
    for status in &statuses {
        // The table is reproduced verbatim, one status per line, so the digit
        // is asserted where it means an exit status rather than anywhere.
        assert!(
            text.contains(&format!("\n  {status}  ")),
            "the man page must document exit status {status}"
        );
    }
}

#[test]
fn the_man_page_is_one_page_for_the_whole_binary() {
    // Subcommands are sections of the single page rather than pages of their
    // own, so there is nothing extra to install or keep in step.
    let text = as_text(&stdout(&run(&["man"])));
    assert!(text.contains(".SH SUBCOMMANDS"));
    assert!(text.contains("openkrx-validate-structure(1)"));
}

#[test]
fn the_document_commands_reject_everything_else() {
    let cases: [&[&str]; 7] = [
        // `completions` needs exactly one shell, and only a shell it knows.
        &["completions"],
        &["completions", "--json"],
        &["completions", "nosuchshell"],
        &["completions", "bash", "zsh"],
        // `man` takes nothing at all.
        &["man", "--json"],
        &["man", "package.krx"],
        &["man", "--out", "openkrx.1"],
    ];
    for args in cases {
        let output = run(args);
        assert_eq!(status(&output), 2, "usage status for {args:?}");
        assert!(output.stdout.is_empty(), "no document for {args:?}");
        assert!(!stderr(&output).is_empty(), "a message for {args:?}");
    }
}

#[test]
fn neither_command_is_a_package_operation() {
    // They describe the binary, not a package: a consumer branching on
    // `operations` must not be told openkrx grew two.
    let text = stdout(&run(&["capabilities", "--json"]));
    assert!(text.contains(
        r#""operations":["inspect","list","validate-structure","extract","create","repack"]"#
    ));
    assert!(!text.contains(r#""completions""#));
    assert!(!text.contains(r#""man""#));
}

#[test]
fn the_help_lists_both_document_commands() {
    let text = stdout(&run(&["--help"]));
    assert!(text.contains("completions"));
    assert!(text.contains("man"));
    // The shell names are the argument's whole domain, so the command's own
    // help must say what may be passed rather than leaving it to a failure.
    let completions = stdout(&run(&["completions", "--help"]));
    for shell in SHELLS {
        assert!(
            completions.contains(shell),
            "the completions help must offer {shell}"
        );
    }
}
