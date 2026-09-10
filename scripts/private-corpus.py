#!/usr/bin/env python3
"""Aggregate-only report over a maintainer-local directory of real packages.

This script is opt-in and local. Nothing runs it: not `scripts/check.sh`, not
CI, not `cargo test`. A maintainer runs it by hand over a directory of real
`.krx` packages that lives outside the repository, reads the counts, and turns
a finding into a *rule* in `docs/profile.md` — never into a fixture, an issue
attachment or a quoted document.

    python3 scripts/private-corpus.py --bin target/release/openkrx --dir DIR
    python3 scripts/private-corpus.py --self-test --bin target/release/openkrx

For every `*.krx` file in the directory it runs `inspect --json`,
`list --json` and `validate-structure --json` under a per-file timeout, and
prints counts alone:

    the number of files; the exit status per command; the `error.code` and
    `error.category` per command; every structural check as
    check x outcome x code-or-rule; the observed root-prefix class; the
    metadata file-name casing class; the format-marker position class; the
    payload subdirectory spelling class; the entry-count histogram; and the
    runs that timed out or produced no envelope.

It never prints a file name, an entry name, a path, a metadata value, a hash,
a timestamp or an identifier. Every label it can print is a fixed string
written in this file: the class names below, the check names, the outcome
names and the stable dotted codes and rule identifiers the CLI already
documents. A value read out of a package is only ever *classified* and then
counted; the value itself is dropped. Errors are counted, never rendered,
because an exception message can carry a path.

The rules this obeys bind the maintainer personally as well as the code, and
are stated in `docs/roadmap.md#private-corpus-policy-for-maintainers` and
`SECURITY.md#data-and-key-policy`. The report is for local reading: it does
not belong in an issue, a pull request, a commit message or a comment.

`--self-test` needs no corpus. It copies the five committed synthetic golden
fixtures into a temporary directory under deliberately loud file names, runs
the very same report over them, asserts the bucket counts those fixtures must
produce, and asserts that none of a canary set — those file names, the
synthetic consignment identifier, the synthetic attachment names, the
metadata entry name — appears anywhere in the output. It exits non-zero on
any failure, and it is the test that keeps the privacy promise honest.
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from collections import Counter
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
FIXTURES = REPO / "tests" / "fixtures" / "golden"

COMMANDS = ("inspect", "list", "validate-structure")
DEFAULT_TIMEOUT = 60.0

# Every class label below is a fixed string. None is derived from a package.
PREFIX_KRX_OCD = "KRX/OCD/"
PREFIX_OCD = "OCD/"
PREFIX_NONE = "none (entries start at the profile directories)"
PREFIX_OTHER = "other prefix"
NOT_OBSERVED = "not observed"

CASE_DOCUMENTED = "upper-case stem, lower-case extension"
CASE_LOWER = "lower-case stem, lower-case extension"
CASE_UPPER = "upper-case stem, upper-case extension"
CASE_OTHER = "other casing"

MARKER_FIRST = "first entry, at the archive root"
MARKER_LATER = "present, not the first entry"
MARKER_PREFIXED = "present under a directory prefix"
MARKER_ABSENT = "no marker entry"

PAYLOAD_HYPHEN = "ID-<n>"
PAYLOAD_BARE = "ID<n>"
PAYLOAD_UNDERSCORE = "ID_<n>"
PAYLOAD_OTHER = "other spelling"
PAYLOAD_NONE = "no payload subdirectory"

ENTRY_BUCKETS = (("1-4", 4), ("5-9", 9), ("10-49", 49), ("50+", None))

MARKER_NAME = "mimetype"
METADATA_DIRECTORY = "Metalayer"
PAYLOAD_DIRECTORY = "Payload"

# The canaries the self-test plants and then hunts for in the report.
CANARY_STEM = "CANARY-NAME"
CANARIES = (
    "SYN-0001",
    "synthetic-a.pdf",
    "synthetic-b.pdf",
    "KULDEMENY_META.xml",
    "KULDEMENY_META",
    METADATA_DIRECTORY,
    PAYLOAD_DIRECTORY,
    MARKER_NAME,
)

HEADER = (
    "openKRX private-corpus report",
    "",
    "Counts only. This report carries no file name, entry name, path,",
    "metadata value, hash, timestamp or identifier; every label in it is a",
    "fixed class, check, outcome, rule or stable error code. It is for local",
    "reading: do not attach it, or anything derived from the corpus, to an",
    "issue, a pull request, a commit or a comment. Report a finding as a rule",
    "in docs/profile.md, citing the class count, never a document.",
)


def environment():
    """A fixed environment, so no local setting can reach the CLI's output."""
    return {
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
        "LC_ALL": "C",
        "TZ": "UTC",
        "NO_COLOR": "1",
    }


class Report:
    """Every counter the report prints, and nothing else.

    Each counter is keyed by a fixed label or by a value the CLI already
    documents as stable — a check name, an outcome, a rule identifier, a
    dotted error code. Package content reaches a counter only after being
    classified into one of the fixed classes above.
    """

    def __init__(self):
        self.files = 0
        self.status = Counter()
        self.codes = Counter()
        self.categories = Counter()
        self.checks = Counter()
        self.prefix = Counter()
        self.casing = Counter()
        self.marker = Counter()
        self.payload = Counter()
        self.entries = Counter()
        self.anomalies = Counter()


def run(binary, command, path, timeout, report):
    """Run one command over one file and return its parsed envelope.

    Returns `None` when there is no envelope to read. The reason is counted,
    never rendered: an exception's text can carry the path it failed on.
    """
    try:
        completed = subprocess.run(
            [str(binary), command, str(path), "--json"],
            capture_output=True,
            env=environment(),
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired:
        report.anomalies[(command, "timed out")] += 1
        return None
    except OSError:
        report.anomalies[(command, "could not be run")] += 1
        return None
    report.status[(command, completed.returncode)] += 1
    try:
        envelope = json.loads(completed.stdout.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        report.anomalies[(command, "no JSON envelope on stdout")] += 1
        return None
    if not isinstance(envelope, dict):
        report.anomalies[(command, "no JSON envelope on stdout")] += 1
        return None
    error = envelope.get("error")
    if isinstance(error, dict):
        report.codes[(command, str(error.get("code", "")))] += 1
        report.categories[str(error.get("category", ""))] += 1
    return envelope


def data_of(envelope):
    """The `data` object of a successful envelope, or `None`."""
    if not envelope or not envelope.get("ok"):
        return None
    data = envelope.get("data")
    return data if isinstance(data, dict) else None


def classify_prefix(prefix):
    """The root-prefix class of one observed prefix (rule A19)."""
    if prefix is None:
        return NOT_OBSERVED
    if prefix == "":
        return PREFIX_NONE
    if prefix == PREFIX_KRX_OCD:
        return PREFIX_KRX_OCD
    if prefix == PREFIX_OCD:
        return PREFIX_OCD
    return PREFIX_OTHER


def classify_casing(name):
    """The metadata file-name casing class of one entry name (rule M12)."""
    if not name:
        return NOT_OBSERVED
    leaf = name.rsplit("/", 1)[-1]
    stem, _, extension = leaf.rpartition(".")
    if not stem:
        return CASE_OTHER
    if stem.isupper() and extension.islower():
        return CASE_DOCUMENTED
    if stem.islower() and extension.islower():
        return CASE_LOWER
    if stem.isupper() and extension.isupper():
        return CASE_UPPER
    return CASE_OTHER


def classify_marker(entries):
    """The format-marker position class of one entry list (rules A2, A19)."""
    for entry in entries:
        name = entry.get("name")
        if not isinstance(name, str):
            continue
        if name == MARKER_NAME:
            return MARKER_FIRST if entry.get("index") == 0 else MARKER_LATER
        if name.endswith("/" + MARKER_NAME):
            return MARKER_PREFIXED
    return MARKER_ABSENT


def classify_payload(entries):
    """The payload subdirectory spelling classes of one entry list (M10).

    A package may spell more than one of them, so this returns the set of
    classes it exhibits and the caller counts each once per package: the
    report says how many packages carry a spelling, never which entries did.
    """
    classes = set()
    for entry in entries:
        name = entry.get("name")
        if not isinstance(name, str):
            continue
        parts = name.split("/")
        for index, part in enumerate(parts[:-1]):
            if part != PAYLOAD_DIRECTORY or index + 1 >= len(parts) - 1:
                continue
            classes.add(payload_class(parts[index + 1]))
    return classes or {PAYLOAD_NONE}


def payload_class(segment):
    """The spelling class of one payload subdirectory name."""
    for prefix, label in (
        ("ID-", PAYLOAD_HYPHEN),
        ("ID_", PAYLOAD_UNDERSCORE),
        ("ID", PAYLOAD_BARE),
    ):
        rest = segment[len(prefix) :]
        if segment.startswith(prefix) and rest.isdigit():
            return label
    return PAYLOAD_OTHER


def classify_entry_count(count):
    """The histogram bucket one entry count falls in."""
    if not isinstance(count, int):
        return NOT_OBSERVED
    for label, upper in ENTRY_BUCKETS:
        if upper is None or count <= upper:
            return label
    return NOT_OBSERVED


def observe(report, inspect, listing, structure):
    """Fold one file's three envelopes into the counters."""
    inspected = data_of(inspect)
    observations = inspected.get("observations", {}) if inspected else {}
    if not isinstance(observations, dict):
        observations = {}
    report.prefix[classify_prefix(observations.get("root_prefix"))] += 1
    report.casing[classify_casing(observations.get("metadata_entry_name"))] += 1
    report.entries[classify_entry_count(observations.get("entry_count"))] += 1

    listed = data_of(listing)
    entries = listed.get("entries") if listed else None
    if isinstance(entries, list):
        entries = [entry for entry in entries if isinstance(entry, dict)]
        report.marker[classify_marker(entries)] += 1
        for label in classify_payload(entries):
            report.payload[label] += 1
    else:
        report.marker[NOT_OBSERVED] += 1
        report.payload[NOT_OBSERVED] += 1

    checked = data_of(structure)
    checks = checked.get("checks") if checked else None
    if not isinstance(checks, list):
        return
    for check in checks:
        if not isinstance(check, dict):
            continue
        detail = check.get("code") or check.get("rule") or ""
        report.checks[
            (str(check.get("check", "")), str(check.get("outcome", "")), str(detail))
        ] += 1


def packages(directory, recursive):
    """The `*.krx` files to read, in a fixed order and case-insensitively.

    Only regular files are read: a symlink to one is followed, a directory
    named `something.krx` is not descended into by accident, and nothing else
    in the directory is opened at all.
    """
    walk = directory.rglob("*") if recursive else directory.iterdir()
    found = []
    for path in walk:
        try:
            if path.suffix.lower() == ".krx" and path.is_file():
                found.append(path)
        except OSError:
            continue
    return sorted(found)


def scan(binary, directory, recursive, timeout):
    """Run every command over every package and return the filled report."""
    report = Report()
    for path in packages(directory, recursive):
        report.files += 1
        envelopes = [run(binary, command, path, timeout, report) for command in COMMANDS]
        observe(report, *envelopes)
    return report


def section(lines, title, rows):
    """Append one titled block of `label: count` rows, sorted for stability."""
    lines.append("")
    lines.append(title)
    if not rows:
        lines.append("  (none)")
        return
    width = max(len(label) for label, _ in rows)
    for label, count in rows:
        lines.append(f"  {label.ljust(width)}  {count}")


def by_label(counter):
    """A counter's rows, ordered by label."""
    return [(label, counter[label]) for label in sorted(counter)]


def by_pair(counter, join=" "):
    """A counter keyed by a tuple, ordered and joined into one label."""
    return [
        (join.join(str(part) for part in key if str(part) != ""), counter[key])
        for key in sorted(counter, key=lambda key: tuple(str(part) for part in key))
    ]


def render(report):
    """The whole report as lines. This is the only thing ever printed."""
    lines = list(HEADER)
    lines.append("")
    lines.append(f"packages read: {report.files}")
    section(lines, "exit status, per command", by_pair(report.status, " exit "))
    section(lines, "error code, per command", by_pair(report.codes))
    section(lines, "error category", by_label(report.categories))
    section(lines, "structural checks: check, outcome, code or rule", by_pair(report.checks))
    section(lines, "root prefix class (A19)", by_label(report.prefix))
    section(lines, "metadata file-name casing class (M12)", by_label(report.casing))
    section(lines, "format-marker position class (A2, A19)", by_label(report.marker))
    section(lines, "payload subdirectory spelling class (M10)", by_label(report.payload))
    section(lines, "entry-count histogram", by_label(report.entries))
    section(lines, "runs with no envelope to read", by_pair(report.anomalies, ": "))
    return lines


def inside_repository(directory):
    """Whether a directory resolves inside this repository's tree."""
    try:
        directory.relative_to(REPO)
    except ValueError:
        return False
    return True


def report_directory(binary, directory, recursive, timeout):
    """Render the report for one directory, returning its lines."""
    return render(scan(binary, directory, recursive, timeout))


def self_test(binary, timeout):
    """Prove the report's buckets and that no canary reaches its output.

    The five committed golden fixtures are copies, so the temporary directory
    holds only synthetic data; the loud file names and the synthetic
    identifiers inside them stand in for the private strings a real corpus
    would carry. A canary in the output is a privacy bug and fails the run.
    """
    with tempfile.TemporaryDirectory() as temporary:
        directory = Path(temporary)
        sources = sorted(FIXTURES.glob("*.krx"))
        if len(sources) != 5:
            print(f"self-test: expected 5 golden fixtures, found {len(sources)}")
            return 1
        for number, source in enumerate(sources, start=1):
            shutil.copyfile(source, directory / f"{CANARY_STEM}-{number}.krx")
        lines = report_directory(binary, directory, False, timeout)
        output = "\n".join(lines)
        failures = check_buckets(output)
        failures += check_canaries(output, directory)
    print(output)
    if failures:
        print(f"\nself-test: {failures} failure(s).")
        return 1
    print("\nself-test: buckets as expected, no canary in the output.")
    return 0


def expected_rows():
    """The rows the five golden fixtures must produce, exactly.

    One fixture is consistent, one is malformed, one references a missing
    attachment, one carries no root prefix, and one exceeds the entry limit —
    so the statuses, the classes and the histogram are all pinned here.
    """
    return (
        "packages read: 5",
        "inspect exit 0  3",
        "inspect exit 6  1",
        "inspect exit 8  1",
        "list exit 0  3",
        "list exit 6  1",
        "list exit 8  1",
        "validate-structure exit 3  1",
        "validate-structure exit 4  2",
        "validate-structure exit 6  1",
        "validate-structure exit 8  1",
        f"{PREFIX_KRX_OCD}  2",
        f"{PREFIX_NONE}  1",
        f"{NOT_OBSERVED}  2",
        f"{CASE_DOCUMENTED}  3",
        f"{MARKER_FIRST}  3",
        f"{PAYLOAD_HYPHEN}  2",
        f"{PAYLOAD_NONE}  1",
        "1-4  3",
        "root_prefix unresolved A19  1",
        "attachment_references fail metadata.reference.missing_entry  1",
        "declared_size unresolved M13  3",
        "inspect archive.malformed.eocd_missing  1",
        "inspect archive.over_limit.entries  1",
        "limit  3",
        "package  3",
    )


def check_buckets(output):
    """Assert every expected row is present, collapsing runs of spaces."""
    rows = [" ".join(line.split()) for line in output.splitlines()]
    failures = 0
    for expected in expected_rows():
        row = " ".join(expected.split())
        if row not in rows:
            print(f"self-test: expected row missing from the report: {row!r}")
            failures += 1
    return failures


def check_canaries(output, directory):
    """Assert no planted string, and no path, reached the report."""
    failures = 0
    planted = [CANARY_STEM, str(directory), directory.name, *CANARIES]
    for canary in planted:
        if canary in output:
            print(
                "self-test: the report leaked a canary. This is a privacy bug "
                f"in scripts/private-corpus.py: {canary!r} must never be "
                "printed, only classified and counted."
            )
            failures += 1
    return failures


def parse_arguments(argv):
    """The command line."""
    parser = argparse.ArgumentParser(
        description=(
            "Aggregate-only report over a local directory of .krx packages. "
            "Opt-in, never run by check.sh or CI, and it prints counts alone."
        )
    )
    parser.add_argument("--bin", required=True, help="the built openkrx executable")
    parser.add_argument("--dir", help="the directory of packages to read")
    parser.add_argument(
        "--recursive",
        action="store_true",
        help="descend into subdirectories; off by default",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=DEFAULT_TIMEOUT,
        help=f"per-file, per-command timeout in seconds (default {DEFAULT_TIMEOUT:g})",
    )
    parser.add_argument(
        "--allow-in-repo",
        action="store_true",
        help="permit a --dir inside the repository tree; refused otherwise",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="run the canary and bucket self-test over the golden fixtures",
    )
    return parser.parse_args(argv)


def main(argv=None):
    arguments = parse_arguments(argv)
    binary = Path(arguments.bin).resolve()
    if not binary.is_file():
        print("the --bin path is not a file; build the executable first")
        return 2
    if arguments.self_test:
        return self_test(binary, arguments.timeout)
    if not arguments.dir:
        print("pass --dir DIRECTORY, or --self-test")
        return 2
    directory = Path(arguments.dir).resolve()
    if not directory.is_dir():
        print("the --dir path is not a directory")
        return 2
    if inside_repository(directory) and not arguments.allow_in_repo:
        print(
            "refusing to read a directory inside the repository tree: a real "
            "corpus belongs outside it, where no commit can reach it. The "
            "conventional place is the gitignored /samples/ directory; pass "
            "--allow-in-repo to read that one deliberately."
        )
        return 2
    for line in report_directory(binary, directory, arguments.recursive, arguments.timeout):
        print(line)
    return 0


if __name__ == "__main__":
    sys.exit(main())
