#!/usr/bin/env python3
"""Validate the recorded and the synthetic JSON envelopes against the schema.

`docs/schema/openkrx-envelope.v1.schema.json` is the machine-readable form of
the command contract in `docs/architecture.md`. This check keeps it honest in
two directions:

* every `tests/golden/<case>/stdout` that parses as JSON must validate, which
  covers each command in both its successful and its failed shape for the
  bytes the golden contract already pins; and
* a small set of failures nothing writes a golden for is produced by running
  the executable itself over synthetic inputs in a temporary directory, so a
  refusal shape that has no recorded case is still checked.

Nothing here reads private data, fetches a URL or executes a package. The
synthetic runs use an empty temporary directory and files this script writes
itself; the schema is loaded from disk and never resolved over the network.

A violation is reported with the case name, the JSON Pointer into the instance
and the schema path that rejected it, and the check exits non-zero on the
first one, because a second report about the same drift adds nothing.

The `jsonschema` PyPI package is required and deliberately not vendored: this
is a repository check, not a runtime dependency of either crate.

    python3 scripts/check-schema.py [--bin PATH]

`--bin` names an already-built executable for the synthetic runs. Without it
the script looks for `target/release/openkrx` and then `target/debug/openkrx`,
and if neither exists it validates the goldens alone and says so.
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SCHEMA = REPO / "docs" / "schema" / "openkrx-envelope.v1.schema.json"
GOLDEN = REPO / "tests" / "golden"
CANDIDATE_BINARIES = (
    Path("target") / "release" / "openkrx",
    Path("target") / "debug" / "openkrx",
)


def environment():
    """A fixed environment, so no local setting can reach an envelope."""
    env = {
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
        "LC_ALL": "C",
        "TZ": "UTC",
        "NO_COLOR": "1",
    }
    if sys.platform == "win32":  # pragma: no cover - CI runs this on Linux
        env["SYSTEMROOT"] = os.environ.get("SYSTEMROOT", "")
    return env


def golden_envelopes():
    """Every golden stdout that parses as JSON, as `(case name, object)`.

    A human-mode golden is not JSON and is skipped; a `--json` golden that
    does not parse is a failure rather than a skip, because the contract says
    stdout carries exactly one JSON object in that mode.
    """
    found = []
    if not GOLDEN.is_dir():
        raise SystemExit(f"no golden cases under {GOLDEN.relative_to(REPO)}")
    for case in sorted(GOLDEN.iterdir(), key=lambda path: path.name):
        stdout = case / "stdout"
        if not stdout.is_file():
            continue
        text = stdout.read_text(encoding="utf-8")
        try:
            found.append((f"tests/golden/{case.name}/stdout", json.loads(text)))
        except json.JSONDecodeError:
            if case.name.endswith("-json"):
                raise SystemExit(
                    f"tests/golden/{case.name}/stdout is a --json case whose "
                    "stdout is not one JSON object; the envelope contract "
                    "says it must be"
                ) from None
    if not found:
        raise SystemExit("no JSON goldens found; check the case layout")
    return found


def synthetic_cases(temp):
    """Failure envelopes rendered by the executable over synthetic inputs.

    Each case names a refusal the golden set does not record. The inputs are
    written here — an empty file that is not an archive, and paths that do not
    exist — so nothing outside this temporary directory is read.
    """
    not_an_archive = temp / "not-an-archive.bin"
    not_an_archive.write_bytes(b"synthetic bytes, not a ZIP image\n")
    absent = temp / "absent-input.krx"
    absent_manifest = temp / "absent-manifest.json"
    absent_edits = temp / "absent-edits.json"
    out = temp / "unwritten-output.krx"
    into = temp / "destination"
    into.mkdir()
    return (
        ("inspect on an unreadable input", ["inspect", str(absent), "--json"]),
        ("list on an unreadable input", ["list", str(absent), "--json"]),
        (
            "validate-structure on bytes that are not an archive",
            ["validate-structure", str(not_an_archive), "--json"],
        ),
        (
            "extract on an unreadable input",
            ["extract", str(absent), "--into", str(into), "--json"],
        ),
        (
            "create with an unreadable manifest",
            ["create", "--manifest", str(absent_manifest), "--out", str(out), "--json"],
        ),
        (
            "repack with an unreadable edits document",
            [
                "repack",
                str(not_an_archive),
                "--edits",
                str(absent_edits),
                "--out",
                str(out),
                "--json",
            ],
        ),
    )


def synthetic_envelopes(binary, temp):
    """Run each synthetic case and return its `(name, object)`.

    A synthetic case must fail: it exists to check a refusal shape, so a run
    that exits 0 means the case no longer describes what it was written for.
    """
    found = []
    for name, arguments in synthetic_cases(temp):
        completed = subprocess.run(
            [str(binary)] + arguments,
            cwd=REPO,
            env=environment(),
            capture_output=True,
            check=False,
        )
        if completed.returncode == 0:
            raise SystemExit(
                f"synthetic case '{name}' exited 0; it is written to produce a "
                "failure envelope, so a success means the case has to be "
                "rewritten rather than the schema relaxed"
            )
        text = completed.stdout.decode("utf-8", "replace")
        try:
            found.append((f"synthetic: {name}", json.loads(text)))
        except json.JSONDecodeError:
            raise SystemExit(
                f"synthetic case '{name}' did not write one JSON object on "
                f"stdout; got {text!r}"
            ) from None
    return found


def pointer(error):
    """The JSON Pointer of the instance location an error is about."""
    return "/" + "/".join(str(part) for part in error.absolute_path)


def validate(validator, name, instance):
    """Report the first violation of one envelope; return True when clean."""
    errors = sorted(validator.iter_errors(instance), key=lambda error: error.path)
    if not errors:
        return True
    error = errors[0]
    print(f"schema violation: {name}")
    print(f"  instance pointer: {pointer(error)}")
    print(f"  schema path: /{'/'.join(str(part) for part in error.absolute_schema_path)}")
    print(f"  {error.message}")
    return False


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--bin",
        help="path to an already-built openkrx executable for the synthetic "
        "failure envelopes; nothing is built here. Without it the usual "
        "target/ paths are tried and the synthetic cases are skipped if "
        "neither exists.",
    )
    parsed = parser.parse_args()

    try:
        from jsonschema import Draft202012Validator
    except ImportError:
        raise SystemExit(
            "the jsonschema package is required by this check; install it "
            "with `pip install jsonschema==4.23.0`"
        ) from None

    schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
    Draft202012Validator.check_schema(schema)
    validator = Draft202012Validator(schema)

    binary = None
    if parsed.bin:
        binary = Path(parsed.bin).resolve()
        if not binary.is_file():
            parser.error(f"no such executable: {binary}")
    else:
        for candidate in CANDIDATE_BINARIES:
            if (REPO / candidate).is_file():
                binary = REPO / candidate
                break

    envelopes = golden_envelopes()
    goldens = len(envelopes)
    temp = Path(tempfile.mkdtemp(prefix="openkrx-schema-"))
    synthetic = 0
    try:
        if binary is not None:
            produced = synthetic_envelopes(binary, temp)
            synthetic = len(produced)
            envelopes += produced
        for name, instance in envelopes:
            if not validate(validator, name, instance):
                print(
                    "the envelope and docs/schema/openkrx-envelope.v1.schema."
                    "json disagree. The schema moves with the envelope: "
                    "update it in the same change, and raise schema_version "
                    "when a field was removed, renamed or redefined."
                )
                return 1
    finally:
        shutil.rmtree(temp, ignore_errors=True)

    if binary is None:
        print(
            f"{goldens} golden envelope(s) validate. Synthetic failure "
            "envelopes skipped: no executable was given with --bin and none "
            "was found under target/."
        )
        return 0
    print(
        f"{goldens} golden and {synthetic} synthetic envelope(s) validate "
        "against docs/schema/openkrx-envelope.v1.schema.json."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
