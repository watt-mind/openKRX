#!/usr/bin/env python3
"""Golden output contract for the openkrx executable.

Every case under `tests/golden/<case>/` is one command run and the exact bytes
it must produce. A case directory holds:

    cmd     one line: the arguments to pass to the executable
    setup   optional; one line of arguments run first, output discarded
    stdout  the exact bytes expected on standard output
    stderr  the exact bytes expected on standard error
    status  the expected exit status, followed by a newline

`cmd` and `setup` are split with `shlex`, so an argument containing spaces is
written quoted, exactly as it would be in a POSIX shell. No other shell
behaviour applies: nothing is expanded, globbed or interpolated, and the
executable is run directly rather than through a shell.

Two placeholders may appear in `cmd` and `setup`:

    {fixture}  the committed fixture directory, `tests/fixtures/golden`,
               written as a repository-relative path so no machine-local
               path can reach a golden
    {outdir}   a fresh, empty directory this case owns, for `extract`

`{outdir}` is substituted already quoted for the shell, so a case writes it
bare — `--into {outdir}` — and it survives a `TMPDIR` containing a space.
Do not quote it again in the case file: `"{outdir}"` would nest the quoting
and pass the quote characters through as part of the argument.

A directory under `tests/golden/` without a `cmd` file is an error, not a
case that is quietly skipped: a case with no command pins nothing, and a
typo in the name would otherwise silently drop coverage.

Subcommands:

    check  --bin PATH   run every case and diff against the committed goldens
    update --bin PATH   run every case and rewrite the committed goldens
    verify-fixtures     regenerate the fixtures and assert they are unchanged

`check` and `update` build nothing and touch no network; the caller supplies
an already-built executable, which CI builds once in the `Golden output
contract` job. `verify-fixtures` is the exception: it runs the fixture
generator through cargo.

No golden may carry an absolute path. The only absolute path any case is
given is its `{outdir}`, so the runner fails loudly if that directory's name
appears on either stream rather than normalising it away. Nothing else is
normalised: the JSON envelope is serialised from Rust structs in declaration
order, so its key order is fixed, and both renderers are pure functions of
the package, so their bytes are stable everywhere.

A difference here is a change to what every consumer parses. Review it
against the `schema_version` rule in `docs/architecture.md` and justify it in
the pull request; do not run `update` to make the build green.
"""

import argparse
import difflib
import filecmp
import os
import shlex
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
GOLDEN = REPO / "tests" / "golden"
FIXTURES = REPO / "tests" / "fixtures" / "golden"
# Repository-relative, because it is substituted into a command line whose
# output is committed. An absolute path here would put this machine in a file.
FIXTURE_RELATIVE = FIXTURES.relative_to(REPO).as_posix()

# The three files a case pins, and the two that drive it.
STREAMS = ("stdout", "stderr", "status")
COMMAND = "cmd"
SETUP = "setup"


def cases():
    """Every case directory, in a fixed order.

    A directory without a `cmd` file is a broken case, not one to skip: it
    would silently pin nothing.
    """
    if not GOLDEN.is_dir():
        return []
    found = sorted(
        (path for path in GOLDEN.iterdir() if path.is_dir()),
        key=lambda path: path.name,
    )
    missing = [path.name for path in found if not (path / COMMAND).is_file()]
    if missing:
        raise SystemExit(
            "contract failure: case director"
            + ("ies" if len(missing) > 1 else "y")
            + " without a "
            f"{COMMAND} file: {', '.join(missing)}. Every directory under "
            "tests/golden/ is one command run; add the missing file or "
            "remove the directory."
        )
    return found


def arguments_of(path, outdir):
    """The argument list one `cmd` or `setup` file describes.

    Split with `shlex`, so an argument that contains spaces can be written
    quoted. `{outdir}` is substituted through `shlex.quote`, because it is a
    machine-local path the case file cannot see: under a `TMPDIR` containing
    a space, a bare `{outdir}` would otherwise split into two arguments. A
    case therefore writes the placeholder bare and must not quote it again —
    quoting it in the file would nest the quoting and leak quote characters
    into the argument. `{fixture}` is a fixed repository-relative path with
    no shell metacharacters, so it is substituted literally.
    """
    line = path.read_text(encoding="utf-8").strip()
    substituted = line.replace("{fixture}", FIXTURE_RELATIVE).replace(
        "{outdir}", shlex.quote(str(outdir))
    )
    return shlex.split(substituted)


def environment():
    """A fixed environment, so no local setting can reach an output."""
    env = {
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
        "LC_ALL": "C",
        "TZ": "UTC",
        "NO_COLOR": "1",
    }
    if sys.platform == "win32":  # pragma: no cover - CI runs this on Linux
        env["SYSTEMROOT"] = os.environ.get("SYSTEMROOT", "")
    return env


def run(binary, case, temp):
    """Run one case and return its `{file name: bytes}`.

    Each case gets an empty directory of its own, so `extract`'s no-clobber
    rule cannot turn a rerun into a different answer, and a case that runs
    `setup` first sees exactly what that run left behind.
    """
    outdir = temp / case.name
    outdir.mkdir()
    setup = case / SETUP
    if setup.is_file():
        prepared = subprocess.run(
            [str(binary)] + arguments_of(setup, outdir),
            cwd=REPO,
            env=environment(),
            capture_output=True,
            check=False,
        )
        if prepared.returncode != 0:
            raise SystemExit(
                f"contract failure: the setup step of case {case.name} "
                f"exited {prepared.returncode}. The case pins what the "
                "command does to the state that step leaves behind, so a "
                "failed setup makes its goldens meaningless.\n"
                "setup stderr:\n" + prepared.stderr.decode("utf-8", "replace")
            )
    completed = subprocess.run(
        [str(binary)] + arguments_of(case / COMMAND, outdir),
        cwd=REPO,
        env=environment(),
        capture_output=True,
        check=False,
    )
    for name, data in (("stdout", completed.stdout), ("stderr", completed.stderr)):
        if str(outdir).encode() in data or str(temp).encode() in data:
            raise SystemExit(
                f"contract failure: {name} of case {case.name} carries the "
                f"temporary directory {outdir}. No output of openkrx may "
                "carry the destination it was given; this is a bug in the "
                "executable, not a normalisation gap in this script."
            )
    return {
        "stdout": completed.stdout,
        "stderr": completed.stderr,
        "status": f"{completed.returncode}\n".encode(),
    }


def committed(case):
    """The committed golden bytes of one case, missing files as `None`."""
    stored = {}
    for name in STREAMS:
        path = case / name
        stored[name] = path.read_bytes() if path.is_file() else None
    return stored


def report(case, name, expected, actual):
    """Print one mismatch as a unified diff over the decoded text."""
    print(f"golden mismatch: tests/golden/{case.name}/{name}")
    if expected is None:
        print("  no golden is committed for this stream")
        return
    diff = difflib.unified_diff(
        expected.decode("utf-8", "replace").splitlines(keepends=True),
        actual.decode("utf-8", "replace").splitlines(keepends=True),
        fromfile=f"tests/golden/{case.name}/{name} (committed)",
        tofile=f"tests/golden/{case.name}/{name} (produced)",
    )
    sys.stdout.writelines(diff)
    print()


def check(binary, temp):
    """Run every case and diff it against the committed goldens."""
    found = cases()
    if not found:
        print("no golden cases found under tests/golden/")
        return 1
    failures = 0
    for case in found:
        produced = run(binary, case, temp)
        stored = committed(case)
        for name in STREAMS:
            if stored[name] == produced[name]:
                continue
            failures += 1
            report(case, name, stored[name], produced[name])
    if failures:
        print(
            f"{failures} golden file(s) differ across {len(found)} case(s). A "
            "difference here is a change to the output contract: review it "
            "against the schema_version rule, justify it in the pull request, "
            "and only then run `python3 scripts/golden.py update --bin <path>`."
        )
        return 1
    print(f"{len(found)} golden case(s) match.")
    return 0


def update(binary, temp):
    """Run every case and rewrite the committed goldens."""
    found = cases()
    if not found:
        print("no golden cases found under tests/golden/")
        return 1
    for case in found:
        for name, data in run(binary, case, temp).items():
            (case / name).write_bytes(data)
    print(f"wrote {len(found) * len(STREAMS)} golden file(s) for {len(found)} case(s).")
    return 0


def verify_fixtures(temp):
    """Regenerate the fixtures and assert the committed ones are identical."""
    produced = temp / "fixtures"
    completed = subprocess.run(
        [
            "cargo",
            "run",
            "--quiet",
            "-p",
            "openkrx-core",
            "--features",
            "synthetic-writer",
            "--example",
            "golden_fixtures",
            "--",
            str(produced),
        ],
        cwd=REPO,
        check=False,
    )
    if completed.returncode != 0:
        print("the fixture generator failed; nothing was compared")
        return 1
    # The whole directory listing, not just `*.krx`: a stray file under
    # tests/fixtures/golden/ is neither compared nor reported otherwise, and
    # a fixture the generator stops emitting must be noticed.
    expected = sorted(path.name for path in FIXTURES.iterdir())
    actual = sorted(path.name for path in produced.iterdir())
    if expected != actual:
        print(f"fixture set differs: committed {expected}, generated {actual}")
        print(
            "tests/fixtures/golden/ holds exactly what the generator emits; "
            "remove any stray file or regenerate the directory."
        )
        return 1
    # Both sides: a directory on the generated side would otherwise reach
    # filecmp.cmp and raise IsADirectoryError instead of failing the contract.
    nested = [
        name
        for name in expected
        if not (FIXTURES / name).is_file() or not (produced / name).is_file()
    ]
    if nested:
        print(f"a fixture entry is not a regular file on one side: {nested}")
        print(
            "tests/fixtures/golden/ and the generator's output must both hold "
            "regular files only."
        )
        return 1
    differing = [
        name
        for name in expected
        if not filecmp.cmp(FIXTURES / name, produced / name, shallow=False)
    ]
    if differing:
        for name in differing:
            print(f"fixture differs from the generator: tests/fixtures/golden/{name}")
        print(
            "The generator must be deterministic and the committed fixtures "
            "must be its output. Re-run it into tests/fixtures/golden and "
            "commit the result with the change that caused it."
        )
        return 1
    print(f"{len(expected)} fixture(s) match the generator byte for byte.")
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("action", choices=("check", "update", "verify-fixtures"))
    parser.add_argument(
        "--bin",
        help="path to an already-built openkrx executable; nothing is built "
        "here. Required by check and update, unused by verify-fixtures.",
    )
    parsed = parser.parse_args()
    binary = None
    if parsed.action in ("check", "update"):
        if not parsed.bin:
            parser.error(f"{parsed.action} requires --bin")
        binary = Path(parsed.bin).resolve()
        if not binary.is_file():
            parser.error(f"no such executable: {binary}")
    temp = Path(tempfile.mkdtemp(prefix="openkrx-golden-"))
    try:
        if parsed.action == "check":
            return check(binary, temp)
        if parsed.action == "update":
            return update(binary, temp)
        return verify_fixtures(temp)
    finally:
        shutil.rmtree(temp, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
