#!/usr/bin/env python3
"""Enforce a per-crate caught-mutant floor over a `cargo mutants` report.

Line coverage says a line ran. Mutation testing says whether a test would
have noticed the line doing something else: `cargo mutants` rewrites one
expression at a time, rebuilds, and reruns the suite. A mutant the suite
still passes against is a **survivor** — a behaviour change nothing caught.

This script reads the report `cargo mutants` writes into its `mutants.out`
directory and, for every crate named in `scripts/mutants-floors.txt`:

- counts the `caught`, `missed`, `timeout` and `unviable` outcomes,
- computes the caught percentage as `caught / (caught + missed)`. Timeouts
  and unviable mutants are reported but kept out of both sides of that
  fraction: a timeout is neither proof the mutant was caught nor a gap in
  the tests, and an unviable mutant never compiled, so no test could have
  run against it,
- compares that percentage with the crate's floor and prints a table.

It exits 1 when any crate is below its floor, 0 when every crate is at or
above it, and 2 when the report or the floors file cannot be used at all. A
crate whose mutants all went unviable or timed out has no score at all, which
is reported as `FAIL (no viable mutants)` and exits 1 rather than reading as a
perfect run.

Preferred input is `outcomes.json`. When it is absent (an interrupted run,
or an older `cargo mutants`), the `caught.txt` / `missed.txt` /
`timeout.txt` / `unviable.txt` listings are read instead; those name a file
and line per mutant, from which the crate is resolved through the workspace
layout. Both paths produce the same table.

Standard library only, so the scheduled workflow needs no Python
dependencies beyond the runner's interpreter.
"""

from __future__ import annotations

import argparse
import json
import os
import sys

DEFAULT_DIR = "mutants.out"
DEFAULT_FLOORS = "scripts/mutants-floors.txt"

# The `summary` strings `cargo mutants` writes into outcomes.json. "Success"
# is the baseline (unmutated) build and test entry, not a mutant outcome.
CAUGHT = "CaughtMutant"
MISSED = "MissedMutant"
TIMEOUT = "Timeout"
UNVIABLE = "Unviable"

LIST_FILES = {
    "caught": CAUGHT,
    "missed": MISSED,
    "timeout": TIMEOUT,
    "unviable": UNVIABLE,
}


class GateError(Exception):
    """A condition that makes the gate unable to judge anything."""


# ---------------------------------------------------------------------------
# Floors file
# ---------------------------------------------------------------------------


def load_floors(path):
    """Return {crate: floor_percent} from a `crate<TAB>floor` file.

    Blank lines and `#` comments are ignored. Whitespace, not only a tab,
    separates the two columns, so a file edited with spaces still reads.
    """
    if not os.path.exists(path):
        raise GateError(f"floors file not found: {path}")
    floors = {}
    with open(path, encoding="utf-8") as handle:
        for number, raw in enumerate(handle, start=1):
            line = raw.split("#", 1)[0].strip()
            if not line:
                continue
            parts = line.split()
            if len(parts) != 2:
                raise GateError(f"{path}:{number}: expected '<crate>\\t<floor>'")
            try:
                floors[parts[0]] = float(parts[1])
            except ValueError:
                raise GateError(
                    f"{path}:{number}: floor '{parts[1]}' is not a number"
                ) from None
    if not floors:
        raise GateError(f"{path}: no crate floors declared")
    return floors


# ---------------------------------------------------------------------------
# Report reading
# ---------------------------------------------------------------------------


def crate_of_path(file_path):
    """Return the crate a mutated source file belongs to, or None.

    Mutant listings carry a workspace-relative path such as
    `crates/openkrx-core/src/archive/eocd.rs`; the crate is the directory
    under `crates/`, which is this workspace's layout (see Cargo.toml).
    """
    parts = file_path.replace("\\", "/").split("/")
    if len(parts) >= 2 and parts[0] == "crates":
        return parts[1]
    return None


def read_outcomes_json(path):
    """Yield (crate, outcome, file, line, description) from outcomes.json."""
    with open(path, encoding="utf-8") as handle:
        data = json.load(handle)
    for outcome in data.get("outcomes", []):
        # The baseline entry's scenario is the bare string "Baseline"; only a
        # mutant's is an object carrying the record read below.
        scenario = outcome.get("scenario")
        if not isinstance(scenario, dict):
            continue
        mutant = scenario.get("Mutant")
        if mutant is None:
            continue
        summary = outcome.get("summary")
        if summary not in (CAUGHT, MISSED, TIMEOUT, UNVIABLE):
            continue
        file_path = mutant.get("file", "")
        crate = mutant.get("package") or crate_of_path(file_path)
        line = mutant.get("span", {}).get("start", {}).get("line", 0)
        name = mutant.get("name", "")
        # `name` is "<file>:<line>:<col>: <description>"; keep the description.
        description = name.split(": ", 1)[1] if ": " in name else name
        yield crate, summary, file_path, line, description


def read_list_files(dir_path):
    """Yield (crate, outcome, file, line, description) from the .txt listings.

    Each line is `<file>:<line>:<col>: <description>`, the same text
    `cargo mutants --list` prints.
    """
    for stem, summary in LIST_FILES.items():
        path = os.path.join(dir_path, f"{stem}.txt")
        if not os.path.exists(path):
            continue
        with open(path, encoding="utf-8") as handle:
            for raw in handle:
                entry = raw.strip()
                if not entry:
                    continue
                head, _, description = entry.partition(": ")
                fields = head.split(":")
                file_path = fields[0]
                try:
                    line = int(fields[1])
                except (IndexError, ValueError):
                    line = 0
                yield (
                    crate_of_path(file_path),
                    summary,
                    file_path,
                    line,
                    description or head,
                )


def read_report(dir_path):
    """Return per-crate tallies and survivor lists from a mutants.out directory."""
    if not os.path.isdir(dir_path):
        raise GateError(f"report directory not found: {dir_path}")
    outcomes_json = os.path.join(dir_path, "outcomes.json")
    if os.path.exists(outcomes_json):
        source = read_outcomes_json(outcomes_json)
        origin = outcomes_json
    else:
        source = read_list_files(dir_path)
        origin = f"{dir_path}/*.txt"

    crates = {}
    total = 0
    for crate, summary, file_path, line, description in source:
        if crate is None:
            continue
        total += 1
        tally = crates.setdefault(
            crate,
            {"caught": 0, "missed": 0, "timeout": 0, "unviable": 0, "survivors": []},
        )
        if summary == CAUGHT:
            tally["caught"] += 1
        elif summary == MISSED:
            tally["missed"] += 1
            tally["survivors"].append((file_path, line, description))
        elif summary == TIMEOUT:
            tally["timeout"] += 1
        elif summary == UNVIABLE:
            tally["unviable"] += 1
    if total == 0:
        raise GateError(f"no mutant outcomes found in {origin}")
    for tally in crates.values():
        tally["survivors"].sort(key=lambda item: (item[0], item[1]))
    return crates, origin


# ---------------------------------------------------------------------------
# Reporting
# ---------------------------------------------------------------------------


def caught_percent(caught, missed):
    """Caught share of the mutants that both compiled and finished, or None.

    `None` means the crate produced no mutant that both compiled and
    finished, so there is no score to compare against a floor. That is a
    failure, not a perfect run: reporting it as 100 % would let a crate whose
    mutants all went unviable — a build that stopped matching the source, a
    `--file` filter that matched nothing — pass the gate in silence.
    """
    denominator = caught + missed
    if denominator == 0:
        return None
    return 100.0 * caught / denominator


def build_report(crates, floors, origin, show_survivors):
    """Return (text, failed_crates) for the gate's output."""
    lines = ["## Mutation testing gate", "", f"Report: `{origin}`", ""]
    lines.append("| Crate | Caught | Missed | Timeout | Unviable | Caught % | Floor | Result |")
    lines.append("| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |")

    failed = []
    for crate in sorted(floors):
        floor = floors[crate]
        tally = crates.get(crate)
        if tally is None:
            failed.append(crate)
            lines.append(
                f"| {crate} | - | - | - | - | - | {floor:.2f}% | FAIL (no mutants in report) |"
            )
            continue
        percent = caught_percent(tally["caught"], tally["missed"])
        if percent is None:
            failed.append(crate)
            result = "FAIL (no viable mutants)"
            score = "-"
        else:
            below = percent < floor
            if below:
                failed.append(crate)
            result = "FAIL" if below else "ok"
            score = f"{percent:.2f}%"
        lines.append(
            f"| {crate} | {tally['caught']} | {tally['missed']} | {tally['timeout']} | "
            f"{tally['unviable']} | {score} | {floor:.2f}% | {result} |"
        )

    ungated = sorted(set(crates) - set(floors))
    if ungated:
        lines.append("")
        lines.append(
            "Crates present in the report with no floor (not gated): "
            + ", ".join(f"`{name}`" for name in ungated)
        )

    if show_survivors:
        for crate in sorted(floors):
            tally = crates.get(crate)
            if not tally or not tally["survivors"]:
                continue
            lines.append("")
            lines.append(f"### Survivors in `{crate}` ({len(tally['survivors'])})")
            lines.append("")
            lines.append("| File | Line | Mutation |")
            lines.append("| --- | ---: | --- |")
            for file_path, line, description in tally["survivors"]:
                lines.append(f"| {file_path} | {line} | {description} |")

    lines.append("")
    if failed:
        lines.append(
            "Not at their floor: " + ", ".join(f"`{name}`" for name in failed) + "."
        )
    else:
        lines.append("Every gated crate is at or above its floor.")
    return "\n".join(lines) + "\n", failed


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main(argv=None):
    parser = argparse.ArgumentParser(
        prog="mutants_gate.py",
        description=(
            "Enforce the per-crate caught-mutant floors in "
            f"{DEFAULT_FLOORS} against a cargo-mutants report."
        ),
        epilog=(
            "Caught percentage is caught / (caught + missed); timeouts and "
            "unviable mutants are reported but counted on neither side. "
            "Exits 1 when a crate is below its floor, 2 when the report or "
            "the floors file cannot be read."
        ),
    )
    parser.add_argument(
        "--dir",
        default=DEFAULT_DIR,
        help=(
            "The mutants.out directory cargo-mutants wrote, i.e. the "
            "directory holding outcomes.json, not its parent "
            f"(default: {DEFAULT_DIR})."
        ),
    )
    parser.add_argument(
        "--floors",
        default=DEFAULT_FLOORS,
        help=f"Per-crate floors file, '<crate>\\t<percent>' per line (default: {DEFAULT_FLOORS}).",
    )
    parser.add_argument(
        "--no-survivors",
        action="store_true",
        help="Print only the per-crate table, without listing each surviving mutant.",
    )
    parser.add_argument(
        "--summary",
        default=os.environ.get("GITHUB_STEP_SUMMARY"),
        help=(
            "Append the report to this file as well as stdout "
            "(default: $GITHUB_STEP_SUMMARY when set)."
        ),
    )
    args = parser.parse_args(argv)

    try:
        floors = load_floors(args.floors)
        crates, origin = read_report(args.dir)
    except GateError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2

    report, failed = build_report(crates, floors, origin, not args.no_survivors)
    sys.stdout.write(report)
    if args.summary:
        with open(args.summary, "a", encoding="utf-8") as handle:
            handle.write(report)

    if failed:
        print(
            "error: no caught-mutant percentage at or above the recorded floor "
            "for: " + ", ".join(failed),
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
