#!/usr/bin/env python3
"""Build the fuzzing seed corpus from the committed golden fixtures.

libFuzzer starts from whatever is already in `fuzz/corpus/<target>/`. With an
empty directory a short run spends its first seconds rediscovering that a KRX
package is a ZIP container at all, and never reaches the code that reads a
metadata document. Seeding the directory from the fixtures under
`tests/fixtures/golden/` puts a well-formed package, a truncated one and a
metadata document in front of the mutator on the first execution, so a bounded
run starts deep instead of shallow.

Nothing produced here is committed: `fuzz/corpus/` is ignored by Git, and this
script is run — by the CI lane, by the weekly campaign and by hand — as the
step before a target runs. It reads only files that are already tracked in
this repository, all of them synthetic by the
[fixture policy](../docs/testing.md#fixture-policy); it never reaches outside
the checkout and never touches the network.

Seeds are written by name, one file per derived input, and an existing file
with the same name is overwritten. Inputs libFuzzer itself added to the corpus
are left alone, so seeding an accumulated corpus is safe and idempotent.

Usage:

    python3 fuzz/seed.py            # build (or refresh) every target's corpus
    python3 fuzz/seed.py --verify   # assert the corpus exists; print counts
"""

from __future__ import annotations

import argparse
import sys
import zipfile
import zlib
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
FIXTURES = REPO / "tests" / "fixtures" / "golden"
CORPUS = REPO / "fuzz" / "corpus"

#: Every fuzz target, in the order `fuzz/Cargo.toml` declares them. A target
#: that this script has no material for still gets its directory, so
#: `--verify` speaks about all five and `cargo fuzz run` never meets a missing
#: path.
TARGETS = (
    "inventory",
    "xml_metadata",
    "structure",
    "extract_plan",
    "create_round_trip",
)

#: The targets that take a whole package as their input: each of them starts
#: from `archive::inventory` over the raw bytes, so every `.krx` fixture is a
#: seed for all three, malformed ones included — a truncated container is
#: exactly the neighbourhood the mutator should explore.
PACKAGE_TARGETS = ("inventory", "structure", "extract_plan")

#: The archive member holding the metadata document, matched case-sensitively
#: on the suffix so both the prefixed (`KRX/OCD/Metalayer/...`) and the
#: unprefixed layout are found.
METADATA_SUFFIX = "Metalayer/KULDEMENY_META.xml"


class SeedingFailure(Exception):
    """A fixture could not be read far enough to derive the seeds it should.

    The message names the fixture by its index in `packages()` and never by
    path: like every other diagnostic in this repository it is written for a
    CI log, and an index is enough for anyone holding the same checkout.
    """


def packages() -> list[Path]:
    """Every committed `.krx` fixture, in a stable order."""
    return sorted(FIXTURES.glob("*.krx"))


def members(package: Path, index: int) -> dict[str, bytes]:
    """The entries of a package, or nothing when it is not a readable ZIP.

    `malformed.krx` is deliberately not a container; it is still a seed for
    the byte-string targets, and simply contributes no extracted member here.

    A container that opens but holds a member whose deflate stream is corrupt
    is a different thing entirely — a fixture that is not what it claims to
    be — and is reported as a seeding failure rather than silently yielding an
    empty set of members.
    """
    try:
        with zipfile.ZipFile(package) as archive:
            return {info.filename: archive.read(info) for info in archive.infolist()}
    except zlib.error as error:
        raise SeedingFailure(
            f"fixture {index} holds a member that does not decompress: {error}"
        ) from None
    except (zipfile.BadZipFile, OSError):
        return {}


def metadata_document(entries: dict[str, bytes]) -> bytes | None:
    """The metadata document of a package, by its documented member name."""
    for name, data in entries.items():
        if name.endswith(METADATA_SUFFIX):
            return data
    return None


def attachments(entries: dict[str, bytes]) -> list[bytes]:
    """The payload members of a package, in archive order."""
    return [
        data
        for name, data in entries.items()
        if "Payload/" in name and not name.endswith("/")
    ]


def write(target: str, name: str, data: bytes) -> None:
    (CORPUS / target).mkdir(parents=True, exist_ok=True)
    (CORPUS / target / name).write_bytes(data)


def build() -> dict[str, int]:
    """Write every derived seed and report how many each target received."""
    for target in TARGETS:
        (CORPUS / target).mkdir(parents=True, exist_ok=True)

    written = {target: 0 for target in TARGETS}
    for index, package in enumerate(packages()):
        stem = package.stem
        raw = package.read_bytes()
        for target in PACKAGE_TARGETS:
            write(target, f"fixture-{stem}", raw)
            written[target] += 1

        entries = members(package, index)
        document = metadata_document(entries)
        if document is not None:
            # The parser's own input, not the container around it: without
            # this the `xml_metadata` target would have to synthesise a whole
            # XML document from random bytes before reaching any of the
            # element handling.
            write("xml_metadata", f"fixture-{stem}-metadata", document)
            written["xml_metadata"] += 1

        # `create_round_trip` reads its bytes through `arbitrary`, so any
        # byte string is a valid seed and none of it is interpreted as a
        # package. Concatenating the document with the payload bytes gives the
        # mutator real text and real file content to draw the header strings
        # and attachment bodies from, deterministically derived from the same
        # fixtures.
        blob = b"".join([document or b"", *attachments(entries)])
        if blob:
            write("create_round_trip", f"fixture-{stem}-parts", blob)
            written["create_round_trip"] += 1

    return written


def counts() -> dict[str, int]:
    """How many inputs each target's corpus directory holds."""
    return {
        target: sum(1 for entry in (CORPUS / target).iterdir() if entry.is_file())
        if (CORPUS / target).is_dir()
        else -1
        for target in TARGETS
    }


def report(totals: dict[str, int]) -> None:
    for target in TARGETS:
        print(f"{target}: {totals[target]}")


def verify() -> int:
    """Assert every target has a non-empty corpus; print counts and nothing else."""
    totals = counts()
    missing = [target for target, total in totals.items() if total <= 0]
    report(totals)
    if missing:
        print(
            "corpus missing or empty for: " + ", ".join(missing) + "; run "
            "`python3 fuzz/seed.py` first",
            file=sys.stderr,
        )
        return 1
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--verify",
        action="store_true",
        help="assert the corpus was built and print the per-target counts only",
    )
    arguments = parser.parse_args()

    if arguments.verify:
        return verify()

    if not FIXTURES.is_dir():
        print(f"no fixture directory at {FIXTURES}", file=sys.stderr)
        return 1
    try:
        build()
    except SeedingFailure as failure:
        # One line on stderr, not a traceback: the caller is a CI step whose
        # log is read by whoever the campaign wakes up.
        print(f"seeding failed: {failure}", file=sys.stderr)
        return 1
    report(counts())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
