#!/usr/bin/env python3
"""Print one CHANGELOG.md section, so the release notes come from the changelog.

The changelog is the release notes: `.github/workflows/release.yml` runs this
script and hands the result to `gh release create --notes-file`. Nothing here
writes, fetches or reasons about versions; it slices the Keep a Changelog file
between one `## [heading]` and the next.

    release-notes.py 1.2.3          print the `## [1.2.3]` section
    release-notes.py --unreleased   print the `## [Unreleased]` section
    release-notes.py --check 1.2.3  validate that section, print nothing
    release-notes.py --check --unreleased

A missing or empty section exits 1, so a tagged release cannot be published
with notes the maintainer never wrote.

`--check <version>` has one documented concession. While the changelog holds
no version section at all -- nothing has ever been released, which is the
current state of this repository -- it accepts the `[Unreleased]` section for
any version and says so on stderr. That concession disappears the moment a
first version heading is added, and it never applies to the printing forms:
`release-notes.py <version>`, which is what the release job runs, always
requires the real section.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

CHANGELOG = Path(__file__).resolve().parent.parent / "CHANGELOG.md"

# Keep a Changelog uses `## [1.2.3] - 2026-01-01` and `## [Unreleased]`. The
# link-reference form `## [1.2.3](...)` is accepted too, so a changelog that
# carries compare links keeps working.
HEADING = re.compile(r"^##\s+\[([^\]]+)\]")
UNRELEASED = "unreleased"


def sections(text: str) -> dict[str, str]:
    """Map each `## [name]` heading to the body that follows it."""
    found: dict[str, str] = {}
    name: str | None = None
    body: list[str] = []
    for line in text.splitlines():
        match = HEADING.match(line)
        if match:
            if name is not None:
                found[name] = "\n".join(body).strip("\n")
            name = match.group(1).strip()
            body = []
            continue
        if name is not None:
            body.append(line)
    if name is not None:
        found[name] = "\n".join(body).strip("\n")
    return found


def lookup(found: dict[str, str], wanted: str) -> str | None:
    """Find a section case-insensitively, so `[unreleased]` matches too."""
    for name, body in found.items():
        if name.casefold() == wanted.casefold():
            return body
    return None


def released_versions(found: dict[str, str]) -> list[str]:
    return [name for name in found if name.casefold() != UNRELEASED]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Print one CHANGELOG.md section for use as release notes."
    )
    parser.add_argument(
        "version",
        nargs="?",
        help="the version whose section to print, without a leading 'v'",
    )
    parser.add_argument(
        "--unreleased",
        action="store_true",
        help="use the [Unreleased] section instead of a version section",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="validate the section and print nothing",
    )
    parser.add_argument(
        "--changelog",
        type=Path,
        default=CHANGELOG,
        help=f"changelog path (default: {CHANGELOG})",
    )
    args = parser.parse_args(argv)

    if args.unreleased and args.version:
        parser.error("give a version or --unreleased, not both")
    if not args.unreleased and not args.version:
        parser.error("give a version or --unreleased")

    try:
        text = args.changelog.read_text(encoding="utf-8")
    except OSError as error:
        print(f"error: cannot read {args.changelog}: {error}", file=sys.stderr)
        return 1

    found = sections(text)
    wanted = "Unreleased" if args.unreleased else args.version
    body = lookup(found, wanted)

    if body is None and not args.unreleased and args.check:
        # The documented concession: no version has ever been released, so
        # there is nothing but [Unreleased] for a version section to be.
        if not released_versions(found) and lookup(found, UNRELEASED):
            print(
                f"note: {args.changelog.name} has no [{wanted}] section and no "
                "released version at all; accepting [Unreleased] for --check. "
                "Move the entries under a version heading before tagging.",
                file=sys.stderr,
            )
            return 0

    if body is None:
        print(
            f"error: {args.changelog.name} has no [{wanted}] section",
            file=sys.stderr,
        )
        return 1
    if not body.strip():
        print(
            f"error: the [{wanted}] section of {args.changelog.name} is empty",
            file=sys.stderr,
        )
        return 1

    if not args.check:
        print(body)
    return 0


if __name__ == "__main__":
    sys.exit(main())
