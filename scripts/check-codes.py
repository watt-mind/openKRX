#!/usr/bin/env python3
"""Keep docs/codes.md in step with the stable codes the crates define.

Every failure openKRX reports carries a dotted code whose head is one of the
heads the HEADS line below names, and those codes are part of the public
contract. This check extracts
each such string literal from the crate sources and compares the set with the
codes docs/codes.md lists in backticks. A code that exists in the sources but
not in the catalogue, or in the catalogue but not in the sources, fails the
check. It also fails when the catalogue names a head HEADS does not, so that
a new head cannot be documented without being extracted. No source is executed
and no private data is read.
"""

from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parent.parent
CATALOGUE = ROOT / "docs" / "codes.md"
# The single source of truth for the stable-code heads. The CLI catalogue test
# `every_catalogued_code_classifies_to_a_category` in
# crates/openkrx-cli/src/exit.rs parses this exact line — one flat tuple of
# double-quoted names on one line — to learn the head set, so a head added here
# alone, or there alone, fails one of the two gates.
HEADS = ("archive", "create", "extract", "input", "metadata", "output")

SEGMENT = r"[a-z0-9_]+"
HEAD = "|".join(HEADS)
SOURCE_CODE = re.compile(rf'"((?:{HEAD})(?:\.{SEGMENT})+)"')
DOC_CODE = re.compile(rf"`((?:{HEAD})(?:\.{SEGMENT})+)`")
# Head-agnostic: any backticked dotted lower-case token in the catalogue.
ANY_DOC_CODE = re.compile(rf"`([a-z]+(?:\.{SEGMENT})+)`")


def source_codes() -> dict[str, set[str]]:
    """Map each code literal to the crate source files that define it."""
    found: dict[str, set[str]] = {}
    for path in sorted(ROOT.glob("crates/*/src/**/*.rs")):
        for code in SOURCE_CODE.findall(path.read_text(encoding="utf-8")):
            found.setdefault(code, set()).add(str(path.relative_to(ROOT)))
    return found


def documented_codes() -> set[str]:
    """Every code docs/codes.md names in backticks."""
    if not CATALOGUE.is_file():
        print(f"missing catalogue: {CATALOGUE.relative_to(ROOT)}")
        sys.exit(1)
    return set(DOC_CODE.findall(CATALOGUE.read_text(encoding="utf-8")))


def main() -> int:
    sources = source_codes()
    if not sources:
        print("no stable codes found in crates/*/src; check the extractor")
        return 1
    documented = documented_codes()
    catalogue = CATALOGUE.relative_to(ROOT)

    problems = 0
    documented_heads = {
        code.split(".", 1)[0]
        for code in ANY_DOC_CODE.findall(CATALOGUE.read_text(encoding="utf-8"))
    }
    for head in sorted(documented_heads - set(HEADS)):
        print(
            f"{catalogue}: documents {head}.* codes, but HEADS in "
            f"{Path(__file__).name} does not name {head}; add it there and the "
            f"catalogue test in crates/openkrx-cli/src/exit.rs will follow"
        )
        problems += 1
    for head in sorted(set(HEADS) - documented_heads):
        print(
            f"{catalogue}: HEADS names {head}, but the catalogue documents no "
            f"{head}.* code; remove the head or catalogue its codes"
        )
        problems += 1
    for code in sorted(set(sources) - documented):
        where = ", ".join(sorted(sources[code]))
        print(f"{catalogue}: undocumented code {code} (defined in {where})")
        problems += 1
    for code in sorted(documented - set(sources)):
        print(f"{catalogue}: documented code {code} exists in no crate source")
        problems += 1

    if problems:
        print(
            f"stable-code catalogue out of date, problems={problems}; "
            f"add or remove the rows in {catalogue} in the same change"
        )
        return 1
    print(f"stable codes checked, {len(sources)} codes catalogued in {catalogue}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
