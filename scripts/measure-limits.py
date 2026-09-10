#!/usr/bin/env python3
"""Measure what the published limits cost, instead of reasoning about it.

`docs/architecture.md` publishes a table of limits. The values in it were
chosen from what the format description implies, and until this script existed
nothing said what a package sitting on one of them actually costs to read:
the roadmap carried "the limits are reasoned, not measured" as a residual
risk.

This script measures it. For every limit it covers it drives the release
executable over two synthetic packages — one sitting exactly at the limit,
one sitting one step past it — with `inspect`, `validate-structure` and
`extract --json`, and records peak resident memory and wall time for each
run. It prints a Markdown table and writes `docs/limits-measured.md`.

The packages are not built here. `cargo run -p openkrx-core --features
synthetic-writer --example limit_packages` builds them from the test-only
writer inside the core crate, which is where the limits, the archive layout
and the metadata grammar live, and it writes a `packages.json` naming each
case, the limit it exercises, where each package sits on that limit's scale
and the stable code the over-limit package must be refused with. This script
reads that file and knows none of it itself. Everything is written into a
temporary directory and nothing is written into the repository except the
report.

**Peak resident memory.** Two paths, and the report always states which one
produced its numbers. On Linux with `/usr/bin/time -v` available, that is
used and `Maximum resident set size` is read from its output. Otherwise,
where the platform has `os.wait4`, each run is spawned directly and the
kernel's own `ru_maxrss` for that one child is read — the portable form of
`resource.getrusage(RUSAGE_CHILDREN)`, scoped to a single process so a
previous run cannot inflate it. Where neither exists, wall time is still
measured and the memory column reads `not measured`.

**The exit status.** Zero is the normal outcome even when a number moved:
this reports drift, it does not gate it. It exits non-zero for exactly two
reasons — a run that crashed (a signal, or a status no command contract
describes), and an over-limit package that was not refused anywhere, which
would mean a documented limit is not enforced end to end. The second doubles
as an end-to-end limit test, which is why the codes are asserted at all.

Standard library only, so the scheduled workflow needs no Python
dependencies beyond the runner's interpreter.
"""

from __future__ import annotations

import argparse
import datetime
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
"""The repository root, resolved from this file rather than the working dir."""

COMMANDS = ("inspect", "validate-structure", "extract")
"""The commands every package is measured with, in report order."""

ACCEPTED_STATUSES = {0, 4}
"""Statuses an at-limit package may exit with.

0 is success. 4 is `structure_unresolved`, which `validate-structure` returns
for every package this repository can build: rule M13 is undecided, so no
package reaches summary `consistent`. 3 — a check *failed* — is not here.
"""

DRIFT_FACTOR = 2.0
"""Ratio at which a number is called drift rather than noise.

A shared runner moves a wall time by a large factor for reasons that have
nothing to do with this repository, so the comparison is deliberately coarse:
it is meant to catch a package that became an order of magnitude more
expensive, not to police a percentage.
"""

MIN_WALL = 0.05
"""Wall time below which a ratio means nothing and drift is not reported.

Half of these packages are measured in single-digit milliseconds, where
process start-up dominates and doubling is what a busy runner does on its own.
"""


# --------------------------------------------------------------- measuring


class Measurement:
    """One run of one command over one package."""

    def __init__(self, status: int, wall: float, rss: int | None, out: bytes):
        self.status = status
        """Exit status, or a negative number for a terminating signal."""
        self.wall = wall
        """Wall time in seconds, measured around the child."""
        self.rss = rss
        """Peak resident set size in bytes, or None when not measured."""
        self.out = out
        """Whatever the command wrote on stdout."""

    @property
    def crashed(self) -> bool:
        """Whether the run ended in a way no command contract describes."""
        return self.status < 0 or self.status > 9

    def codes(self) -> set[str]:
        """Every stable code the JSON envelope carries, anywhere in it."""
        try:
            envelope = json.loads(self.out.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError):
            return set()
        return collect_codes(envelope)


def collect_codes(node: object) -> set[str]:
    """Collect every `code` value in a decoded envelope.

    A refusal carries its code in `error.code`; a metadata document over a
    metadata limit is not a refusal at all but a failed structural check, and
    carries the same code in that check's `code`. Both are the limit being
    enforced, so both count.
    """
    found: set[str] = set()
    if isinstance(node, dict):
        code = node.get("code")
        if isinstance(code, str):
            found.add(code)
        for value in node.values():
            found |= collect_codes(value)
    elif isinstance(node, list):
        for value in node:
            found |= collect_codes(value)
    return found


def rss_scale() -> int:
    """Bytes per unit of `ru_maxrss`, which Linux reports in kibibytes."""
    return 1 if sys.platform == "darwin" else 1024


def measure_with_time(argv: list[str]) -> Measurement:
    """Run `argv` under `/usr/bin/time -v` and read its resident-size line."""
    start = time.perf_counter()
    done = subprocess.run(
        ["/usr/bin/time", "-v", *argv],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    wall = time.perf_counter() - start
    match = re.search(
        rb"Maximum resident set size \(kbytes\): (\d+)", done.stderr
    )
    rss = int(match.group(1)) * 1024 if match else None
    return Measurement(done.returncode, wall, rss, done.stdout)


def measure_with_wait4(argv: list[str]) -> Measurement:
    """Spawn `argv` and read the kernel's `ru_maxrss` for that one child."""
    with tempfile.TemporaryFile() as out, open(os.devnull, "wb") as null:
        start = time.perf_counter()
        pid = os.posix_spawn(
            argv[0],
            argv,
            os.environ,
            file_actions=[
                (os.POSIX_SPAWN_DUP2, out.fileno(), 1),
                (os.POSIX_SPAWN_DUP2, null.fileno(), 2),
            ],
        )
        _, status, usage = os.wait4(pid, 0)
        wall = time.perf_counter() - start
        out.seek(0)
        payload = out.read()
    code = -os.WTERMSIG(status) if os.WIFSIGNALED(status) else (status >> 8)
    return Measurement(code, wall, usage.ru_maxrss * rss_scale(), payload)


def choose_method() -> tuple[str, object]:
    """The measurement path this machine offers, and how it is described."""
    if sys.platform.startswith("linux") and Path("/usr/bin/time").exists():
        return "/usr/bin/time -v", measure_with_time
    if hasattr(os, "wait4") and hasattr(os, "posix_spawn"):
        return "os.wait4 (ru_maxrss for the measured child)", measure_with_wait4
    return "wall time only: no resident-size source on this platform", None


def measure(method: object, argv: list[str]) -> Measurement:
    """Run `argv`, with peak memory when the platform offers a source."""
    if method is None:
        start = time.perf_counter()
        done = subprocess.run(
            argv, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=False
        )
        return Measurement(
            done.returncode, time.perf_counter() - start, None, done.stdout
        )
    return method(argv)  # type: ignore[operator]


# ---------------------------------------------------------------- the runs


def run(command: str, exe: Path, package: Path, work: Path, method) -> Measurement:
    """Measure one command over one package, with a fresh destination."""
    argv = [str(exe), command, str(package), "--json"]
    if command == "extract":
        destination = work / "extracted"
        shutil.rmtree(destination, ignore_errors=True)
        destination.mkdir(parents=True)
        argv += ["--into", str(destination)]
    result = measure(method, argv)
    if command == "extract":
        shutil.rmtree(work / "extracted", ignore_errors=True)
    return result


class Row:
    """One package's measurement across every command."""

    def __init__(self, case: dict, side: str, runs: dict[str, Measurement]):
        self.case = case
        self.side = side
        """Either `at` or `over`."""
        self.runs = runs

    @property
    def rss(self) -> int | None:
        """The largest peak resident size any of the commands reached."""
        values = [run.rss for run in self.runs.values() if run.rss is not None]
        return max(values) if values else None

    @property
    def codes(self) -> set[str]:
        """Every code any of the commands reported."""
        found: set[str] = set()
        for measurement in self.runs.values():
            found |= measurement.codes()
        return found

    def failures(self) -> list[str]:
        """Everything about this row that must end the run non-zero."""
        problems = [
            f"{self.case['dimension']} ({self.side}): {name} ended abnormally "
            f"with status {measurement.status}"
            for name, measurement in self.runs.items()
            if measurement.crashed
        ]
        if self.side == "over" and self.case["code"] not in self.codes:
            problems.append(
                f"{self.case['dimension']} (over): no command reported "
                f"{self.case['code']}, so the limit was not enforced"
            )
        return problems

    def result(self) -> str:
        """The outcome column: what happened to this package, in words."""
        if self.side == "at":
            unexpected = [
                f"{name} exited {measurement.status}"
                for name, measurement in self.runs.items()
                if measurement.status not in ACCEPTED_STATUSES
            ]
            return "accepted" if not unexpected else "; ".join(unexpected)
        # Which commands *reported the code*, not which exited non-zero:
        # `validate-structure` exits 4 for every package this repository can
        # build, and a package one step over a metadata limit is a failed
        # structural check rather than a refusal.
        reporting = [
            name
            for name, measurement in self.runs.items()
            if self.case["code"] in measurement.codes()
        ]
        return f"`{self.case['code']}` from {', '.join(reporting)}"


# ------------------------------------------------------------- the reporting


def human_bytes(value: int | None) -> str:
    """A resident size in mebibytes, or a note that none was measured."""
    if value is None:
        return "not measured"
    return f"{value / (1024 * 1024):.1f} MiB"


def human_measured(case: dict, side: str) -> str:
    """Where a package sits on its limit's scale, with the unit."""
    return f"{case[side]['measured']} {case['unit']}"


def table(rows: list[Row]) -> str:
    """The measurement as one Markdown table."""
    header = (
        "| Limit | Default | Package | Measured | "
        + " | ".join(f"{name} (s)" for name in COMMANDS)
        + " | Peak RSS | Result |\n"
    )
    header += "| --- " * (5 + len(COMMANDS)) + "| --- |\n"
    lines = []
    for row in rows:
        times = " | ".join(
            f"{row.runs[name].wall:.2f}" for name in COMMANDS
        )
        lines.append(
            f"| `{row.case['limit']}` | {row.case['limit_value']} | "
            f"{'at the limit' if row.side == 'at' else 'one step over'} | "
            f"{human_measured(row.case, row.side)} | {times} | "
            f"{human_bytes(row.rss)} | {row.result()} |"
        )
    return header + "\n".join(lines) + "\n"


def cpu_model() -> str:
    """The CPU this ran on, read from the platform's own description."""
    if sys.platform.startswith("linux"):
        try:
            for line in Path("/proc/cpuinfo").read_text().splitlines():
                if line.startswith("model name"):
                    return line.split(":", 1)[1].strip()
        except OSError:
            pass
    if sys.platform == "darwin":
        try:
            return subprocess.check_output(
                ["sysctl", "-n", "machdep.cpu.brand_string"], text=True
            ).strip()
        except (OSError, subprocess.SubprocessError):
            pass
    return platform.processor() or "unknown"


def commit() -> str:
    """The commit measured, or `unknown` outside a checkout."""
    try:
        return subprocess.check_output(
            ["git", "-C", str(ROOT), "rev-parse", "HEAD"], text=True
        ).strip()
    except (OSError, subprocess.SubprocessError):
        return "unknown"


def document(rows: list[Row], method: str) -> str:
    """The whole of `docs/limits-measured.md`."""
    date = datetime.datetime.now(datetime.timezone.utc).date().isoformat()
    return f"""# Measured limits

The limit table in [architecture.md](architecture.md#limits) is the contract.
This document is one measurement of what sitting on that contract costs, taken
by `scripts/measure-limits.py` on one machine, on one day.

**These numbers are a measurement, not a guarantee.** They describe the
machine named below and nothing else: another CPU, another allocator, another
operating system or another build profile will produce different ones, and no
number here is a promise about any run anywhere. What the table *does* hold is
the right-hand column — every package built one step past a limit was reported
with the stable code that limit publishes, and every package built exactly on
a limit was accepted. That part is a property of this repository rather than
of this machine, and `scripts/measure-limits.py` fails when it stops holding.

| Fact | Value |
| --- | --- |
| Commit | `{commit()}` |
| Date | {date} |
| Operating system | {platform.platform()} |
| CPU | {cpu_model()} |
| Python | {platform.python_version()} |
| Peak-memory source | `{method}` |

## What was measured

For each limit, the generator
`crates/openkrx-core/examples/limit_packages.rs` writes two synthetic
packages: one sitting exactly at the limit, one sitting one step past it.
Every one of them is driven through `inspect`, `validate-structure` and
`extract --json`. The wall-time columns are per command; the peak resident
size is the largest any of the three reached, because that is the number a
caller sizing a container cares about.

{table(rows)}
"Peak RSS" is the whole process: the executable, its runtime and the bounded
buffer it reads the package into. `validate-structure` returning exit 4 is the
normal outcome for every package this repository can build — rule M13 is
undecided, so `consistent` is unreachable — and is not a finding.

## What these numbers are not

They are not a performance contract, a capacity plan or evidence about any
real package. They are not a conformance, validity or interoperability
statement: structural checking is not signature verification, and openKRX
performs no cryptography. The packages measured are synthetic originals
written by this repository, as [the fixture
policy](../tests/fixtures/README.md) requires, and none of them is a
conforming KRX file.

## Re-running it

```sh
python3 scripts/measure-limits.py
```

It builds the release executable and the generator, writes every package into
a temporary directory outside the repository, measures, rewrites this
document and removes the packages. Nothing is left behind and nothing large is
committed. `--compare` reports drift against a previous run of the same
document without changing the exit status; see
[testing.md](testing.md#measuring-the-limits).
"""


def parse_table(text: str) -> dict[tuple[str, str], list[str]]:
    """The rows of a previous report, keyed by limit and package."""
    rows: dict[tuple[str, str], list[str]] = {}
    for line in text.splitlines():
        if not line.startswith("| `"):
            continue
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        if len(cells) >= 4:
            rows[(cells[0], cells[2])] = cells
    return rows


def drift(rows: list[Row], previous: Path) -> str:
    """A drift report against a previous document, as Markdown."""
    try:
        old = parse_table(previous.read_text())
    except OSError:
        return f"\nNo previous measurement at `{previous}` to compare with.\n"
    lines = []
    for row in rows:
        key = (
            f"`{row.case['limit']}`",
            "at the limit" if row.side == "at" else "one step over",
        )
        before = old.get(key)
        if before is None:
            lines.append(f"- {key[0]} ({key[1]}) is new in this run.")
            continue
        if before[-1] != row.result():
            lines.append(
                f"- {key[0]} ({key[1]}): the result was `{before[-1]}` and is "
                f"now `{row.result()}`."
            )
        lines += drift_numbers(row, before, key)
    if not lines:
        return "\nNo drift: every row matches the checked-in measurement.\n"
    return "\n" + "\n".join(lines) + "\n"


def drift_numbers(row: Row, before: list[str], key: tuple[str, str]) -> list[str]:
    """Wall times that moved by more than [`DRIFT_FACTOR`], as Markdown."""
    lines = []
    for offset, name in enumerate(COMMANDS):
        try:
            was = float(before[4 + offset])
        except (IndexError, ValueError):
            continue
        now = row.runs[name].wall
        if max(was, now) < MIN_WALL or min(was, now) <= 0:
            continue
        if now / was >= DRIFT_FACTOR or was / now >= DRIFT_FACTOR:
            lines.append(
                f"- {key[0]} ({key[1]}): `{name}` took {was:.2f} s and now "
                f"takes {now:.2f} s."
            )
    return lines


# -------------------------------------------------------------------- main


def build(work: Path) -> Path:
    """Build the executable and the generator, and return the executable."""
    subprocess.run(
        ["cargo", "build", "--release", "--locked", "-p", "openkrx-cli"],
        cwd=ROOT,
        check=True,
    )
    subprocess.run(
        [
            "cargo",
            "run",
            "--release",
            "--locked",
            "-p",
            "openkrx-core",
            "--features",
            "synthetic-writer",
            "--example",
            "limit_packages",
            "--",
            str(work),
        ],
        cwd=ROOT,
        check=True,
    )
    return ROOT / "target" / "release" / (
        "openkrx.exe" if os.name == "nt" else "openkrx"
    )


def main() -> int:
    """Measure every case, write the report, and report drift."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--output",
        type=Path,
        default=ROOT / "docs" / "limits-measured.md",
        help="where to write the report (default: docs/limits-measured.md)",
    )
    parser.add_argument(
        "--summary",
        type=Path,
        default=None,
        help="append the table, and any drift report, to this file",
    )
    parser.add_argument(
        "--compare",
        type=Path,
        default=None,
        help="report drift against a previous report, without failing on it",
    )
    arguments = parser.parse_args()

    name, method = choose_method()
    print(f"peak memory measured with {name}", flush=True)
    with tempfile.TemporaryDirectory(prefix="openkrx-limits-") as temporary:
        work = Path(temporary)
        executable = build(work)
        cases = json.loads((work / "packages.json").read_text())["cases"]
        rows = []
        for case in cases:
            for side in ("at", "over"):
                package = work / case[side]["file"]
                runs = {
                    command: run(command, executable, package, work, method)
                    for command in COMMANDS
                }
                rows.append(Row(case, side, runs))
                print(f"measured {case['dimension']} ({side})", flush=True)

    report = table(rows)
    print("\n" + report)
    # Compared before the report is written, so that `--compare` naming the
    # document `--output` is about to overwrite still compares with the
    # checked-in numbers rather than with this run's own.
    drifted = drift(rows, arguments.compare) if arguments.compare else ""
    arguments.output.write_text(document(rows, name))
    print(f"wrote {arguments.output}")

    if drifted:
        print("Drift against the checked-in measurement:" + drifted)
    if arguments.summary:
        with arguments.summary.open("a", encoding="utf-8") as summary:
            summary.write("### Limits measurement\n\n" + report)
            if drifted:
                summary.write(
                    "\n### Drift against the checked-in measurement\n" + drifted
                )

    problems = [problem for row in rows for problem in row.failures()]
    for problem in problems:
        print(f"error: {problem}", file=sys.stderr)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
