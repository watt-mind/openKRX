# Testing and quality gates

Everything openKRX asserts about a format rule is held by a named test. This
document says which test file holds what, how to run the suite and the
coverage gate, what the sweeps guarantee while no fuzz target exists, and the
rules for fixtures and for any private corpus.

## Test layout

Almost every test is an integration test, because the contract worth testing
is the public one: a byte slice goes in, a typed value or a stable code comes
out. The exceptions are the doctests in the public API documentation and one
`#[cfg(test)]` module in `crates/openkrx-core/src/archive/inflate.rs`, which
pins the CRC-32 implementation against its published check value, the empty
input, and chunk ordering — an internal helper with no public surface to
exercise it through.

| File | Covers |
| --- | --- |
| `crates/openkrx-core/tests/archive_inventory.rs` | Archives the inventory accepts and the observations it reports: central-directory ordering, declared metadata beside counted decoded size, the UTF-8 flag reported independently of whether the name decodes, empty and zero-length and directory entries, `entry_bytes` re-decoding one entry, data descriptors with and without their signature, tolerated comments and extra fields, that `Limits::DEFAULT` is the documented table, and that `Display` prints codes and numbers but never an entry name. |
| `crates/openkrx-core/tests/archive_rejects_structure.rs` | Archives that contradict themselves: a missing, duplicated or displaced end record, a wrong declared record count, a missing signature, local headers and data descriptors disagreeing with their record, prefix, trailing, unclaimed and overlapping bytes, CRC and declared-size mismatches, corrupt and truncated deflate streams, ZIP64 markers and multi-disk end records. Holds the truncation sweep and the single-byte mutation sweep. |
| `crates/openkrx-core/tests/archive_rejects_input.rs` | Input the limits and name rules refuse: every limit at its boundary (below, at, above), deflate bombs stopped by the ratio, per-entry and total ceilings, the ratio grace window, every unsafe-name class, byte-identical and case-folded name collisions, unsupported methods, encryption and patched-data flags, ZIP64 records, and a record on another disk. |
| `crates/openkrx-core/tests/metadata_parse.rs` | Documents the parser accepts and the facts it reports: prefixed and default namespace bindings, every enumeration token the schema lists, optional header and attachment elements present and absent, the joined attachment path and its edge cases, and the values the sources leave open — an absent `TESZT`, an absent `MELLEKLET_LEIRASA`, a non-numeric or infinite `MERET` — which are recorded rather than rejected. Also pins `MetadataLimits::DEFAULT` and the target-namespace constant. |
| `crates/openkrx-core/tests/metadata_rejects.rs` | Every refused XML feature (DTD, undeclared entity, forbidden character reference, stray processing instruction, non-UTF-8 declaration and non-UTF-8 bytes, unbound prefix, repeated attribute), every grammar violation (wrong root, wrong namespace, missing, repeated and misordered elements, unexpected child, enumeration, integer and boolean values), and every metadata limit at its boundary. Holds its own truncation and mutation sweeps, and the content-free `Display` assertion. |
| `crates/openkrx-core/tests/profile_structure.rs` | The check inventory over synthetic KRX-shaped archives: the canonical layout, all three observed root prefixes, both metadata file-name spellings, a lower-case `Metalayer`, missing and ambiguous metadata, a missing, misplaced, mismatched and prefixed marker, a deflated marker producing no finding, missing, prefix-variant and duplicate references, a shared file name in different payload directories, count agreement and its absence, an omitted schema-required element, a document that does not parse, caller-tightened limits, and that every check is reported exactly once in the documented order. |
| `crates/openkrx-core/tests/metadata_evidence.rs` | The independent-evidence layer: a synthetic re-expression of the *structure* of the two official sample documents (rules M9 and M10), parsed into the documented shape and resolved inside the documented layout. |
| `crates/openkrx-core/tests/support/` | Test-only synthetic writers, not tests. `mod.rs` builds ZIP images and can emit contradictory headers on purpose; `meta.rs` builds metadata documents from values written from scratch for this repository. |
| `crates/openkrx-cli/tests/contract.rs` | The executable contract by subprocess: the JSON envelope's exact shape, that the human output promises no processing, that help and version are available, and that a missing or unimplemented command is rejected. |

Each rejection test asserts a **stable code**, not merely that an error
occurred, so one rejection category cannot silently become another.
[SECURITY.md](../SECURITY.md#threat-model-mapping-archive-layer) maps each
required threat-model check to its code prefix and test, and
[conformance.md](conformance.md) maps each profile rule to the test that
holds it.

What the suite does **not** provide evidence about: extraction, which does
not exist; conformance, which cannot be claimed while
[profile.md](profile.md#unresolved-essential-rules) lists unresolved rules;
and agreement with a real service, for which no sample or reference
implementation exists.

## Running the tests and coverage

```sh
bash scripts/check.sh
```

That is the gate. It runs, in order: `cargo fmt --all --check`;
`cargo clippy --workspace --all-targets --locked -- -D warnings`;
`cargo test --workspace --locked`; `cargo doc --workspace --no-deps --locked`
with `RUSTDOCFLAGS="-D warnings"`; `cargo machete`;
`cargo deny --all-features check`; `python3 scripts/check-doc-links.py`;
`python3 scripts/check-codes.py`; `python3 scripts/check-file-length.py`;
markdownlint; and `actionlint`.

To run one part by hand:

```sh
cargo test --workspace --locked
cargo test --workspace --locked profile_structure
cargo test -p openkrx-core --locked -- --nocapture single_byte_mutations
```

Coverage:

```sh
cargo llvm-cov --workspace --locked --all-features --summary-only
cargo llvm-cov --workspace --locked --all-features --html
```

The workspace line-coverage floor is **90 %**, enforced in CI. It is a floor,
not a target: parser and filesystem work must add meaningful boundary
coverage rather than only raise the aggregate. A test that merely repeats a
trivial implementation detail raises the number without adding evidence and
should not be written.

Three checks are Python, standard library only, and run offline:

| Script | What it enforces |
| --- | --- |
| `scripts/check-doc-links.py` | Every relative Markdown link resolves, and every heading anchor it names exists. External links are never fetched, so the check stays deterministic. |
| `scripts/check-file-length.py` | Tracked Rust files stay under 800 lines, or 1 500 for test files. |
| `scripts/check-codes.py` | Every `archive.*` and `metadata.*` code literal in `crates/*/src/**` appears in [codes.md](codes.md), and every code that document lists still exists in the sources. |

`scripts/check-codes.py` is the maintenance gate for the code catalogue. It
fails in both directions and names the offending code and the file that
defines it, so adding a code without cataloguing it, or renaming one and
leaving a stale row behind, both stop the build. Run it alone with
`python3 scripts/check-codes.py`; on success it prints one line naming how
many codes are catalogued.

## Sweeps

Two exhaustive sweeps stand in for the fuzz targets that do not exist yet.
They are cheap, deterministic, and run in the normal test suite.

**Truncation.** For each representative input, every prefix — every length
from zero up to one byte short of the whole — is fed to the parser, and each
must be rejected without panicking. The archive sweep covers a marker-only
archive, an empty archive, a two-entry archive with a data descriptor, and an
archive with a comment. The metadata sweep does the same for a valid
document, and separately truncates every *refused* document as well. A third
sweep truncates a whole KRX-shaped archive through `profile::check`.

What that guarantees: no partial structure can be accepted as a complete one,
and no length-driven index can run past the end of the slice. Combined with
the exact-coverage rule, it also means a valid archive cannot be made valid
again by cutting it short.

**Single-byte mutation.** Every byte position of a valid image is replaced,
in turn, with each of `0x00`, `0xff` and `0x50` (the `P` of a ZIP
signature), and the parser must return — accept or reject — without
panicking. `0x50` is included deliberately: it manufactures stray record
signatures inside entry data, which is the shape that confuses a reader that
scans for signatures rather than following the declared structure.

What that guarantees: no reachable arithmetic overflow, slice index or
`unwrap` on any input one byte away from a valid one. It does not guarantee
anything about inputs two bytes away; that is what a fuzz target is for.

## Fixture policy

No binary fixture is committed, and the policy is stated in
[tests/fixtures/README.md](../tests/fixtures/README.md).

Every archive and every document a test reads is generated at run time by
`crates/openkrx-core/tests/support/`, an independent original written for
this repository that can emit contradictory headers on purpose. Generating
beats committing: the property under test is visible in the test that builds
the input, and no opaque binary has to be trusted or re-derived.

A binary fixture may be added only if a property genuinely cannot be
generated. It must then be recorded in the fixture inventory with its
provenance, the property under test and the expected result, and it must be
an independently authored synthetic original — never a real submission, and
never a redacted or modified derivative of one. Third-party example packages
and schemas need documented redistribution permission before they may be
copied here; public accessibility is not permission.

`metadata_evidence.rs` is the one place that touches official material, and
it touches only its shape: an XML declaration, the target namespace bound to
an `ns2` prefix, and a reference split across `ELHELYEZKEDES` and
`FAJL_NEV`. No identifier, description, timestamp, barcode or note is copied
from any source, and none may be added.

## Fuzzing (planned)

Neither parser is fuzzed yet. The sweeps above are the compensating control,
and this is recorded as a residual risk in
[roadmap.md](roadmap.md#residual-risks-in-the-current-state).

Two targets belong in that work, and the attack surfaces for both already
exist:

- `zip_inventory`: `archive::inventory(data, &Limits::DEFAULT)`, asserting
  only that the call returns.
- `xml_metadata`: `metadata::parse(data, &MetadataLimits::DEFAULT)`, the
  same.

Adding a target means: a `fuzz/` cargo-fuzz workspace excluded from the main
workspace; one harness file per target whose body is the single call above;
a seed corpus built by the existing `tests/support/` writers rather than
committed as binaries; a CI lane that runs each target for a bounded time on
pull requests and longer on a schedule; and a rule that every crash is
minimised, converted into a named regression test in the matching
`*_rejects_*.rs` file, and only then fixed. The minimised input goes into the
test as a generated construction, not as a committed blob, whenever it can
be expressed that way.

## Compatibility testing later

Once a reader command exists, a compatibility layer becomes possible and
becomes necessary: openKRX is strict on purpose, and strictness has to be
measurable against what real producers emit.

The shape it must take: a request carries a synthetic reproducer, the
expected profile and version, a public source for the rule in question, and
the expected behaviour. An unsupported profile must stay distinguishable
from malformed input and from a resource-limit refusal, which is why the
code categories exist. A finding is resolved either by a documented, tested
exception with its own stable code, or by a recorded decision not to accept
that input — never by relaxing a check to increase compatibility.

A reader/writer round-trip is explicitly not compatibility evidence: it
proves only that our own components agree with each other. Real evidence
needs an official sample archive or an independent implementation, neither
of which was found; see
[conformance.md](conformance.md#known-gaps).

## Private-corpus policy

No private-corpus harness exists, and none may be added casually. If one is
ever authorised, the rules are in
[roadmap.md](roadmap.md#private-corpus-policy-for-maintainers) and
[SECURITY.md](../SECURITY.md#data-and-key-policy): opt-in, excluded from
public CI, and reporting aggregate counts and stable error-code buckets
only. No filename, path, metadata field, payload, certificate identity or
hash may enter a report, an issue, a commit or a pull request.
