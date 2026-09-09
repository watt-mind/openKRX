#!/usr/bin/env python3
"""Keep docs/codes.md in step with the stable codes the crates define.

Every failure openKRX reports carries a dotted `archive.*`, `extract.*`,
`input.*` or `metadata.*` code, and those codes are part of the public
contract. This check extracts
each such string literal from the crate sources and compares the set with the
codes docs/codes.md lists in backticks. A code that exists in the sources but
not in the catalogue, or in the catalogue but not in the sources, fails the
check. No source is executed and no private data is read.
"""

from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parent.parent
CATALOGUE = ROOT / "docs" / "codes.md"
SEGMENT = r"[a-z0-9_]+"
SOURCE_CODE = re.compile(rf'"((?:archive|extract|input|metadata)(?:\.{SEGMENT})+)"')
DOC_CODE = re.compile(rf"`((?:archive|extract|input|metadata)(?:\.{SEGMENT})+)`")


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
