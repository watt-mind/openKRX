# Testing and quality gates

## Foundation checks

Run `bash scripts/check.sh` locally. CI workflow files define the exact
commands for formatting, Clippy, Rust documentation, Markdown/local links,
file length, tests on Linux/macOS/Windows, MSRV, cargo-deny, cargo-machete,
coverage, and security scanning. The initial workspace line-coverage floor
is 90%. Crates are unpublished and tests use the committed lockfile.

Current tests cover the capability model and executable contract, including
JSON shape, help/version, and argument rejection, plus the bounded archive
inventory, bounded metadata parsing, and the structural check inventory. They
provide no evidence about extraction or profile conformance: extraction does
not exist, and conformance cannot be claimed while
[profile.md](profile.md#unresolved-essential-rules) lists unresolved rules.

## Implemented test layers

The archive inventory is covered by `crates/openkrx-core/tests/`:
`archive_inventory.rs` for accepted archives and reported observations,
`archive_rejects_structure.rs` for contradictory records, ambiguity, coverage
gaps, truncation and corrupt streams, and `archive_rejects_input.rs` for limit
boundaries, unsafe names, collisions and unsupported features. Every archive
is generated programmatically by the test-only writer in `tests/support/`,
which deliberately permits contradictory headers; no binary fixture is
committed. Each test asserts a stable error code, so one rejection category
cannot silently become another, and
[SECURITY.md](../SECURITY.md#threat-model-mapping-archive-layer) maps every
required check to its code prefix and test.

Two sweeps stand in for the fuzz target that does not exist yet: every prefix
of every representative archive must be rejected without panicking, and every
single-byte mutation of a valid archive must return without panicking.

The metadata and profile layers are covered by three more files.
`metadata_parse.rs` holds accepted documents and the facts the parser reports,
including the values `profile.md` leaves open — an absent `TESZT`, an absent
`MELLEKLET_LEIRASA`, a non-numeric `MERET` — which are recorded rather than
rejected. `metadata_rejects.rs` holds every prohibited XML feature, every
grammar violation and every limit boundary, each asserting a stable code, plus
its own truncation and single-byte-mutation sweeps. `profile_structure.rs`
holds the check inventory over synthetic KRX-shaped archives: all three
observed layouts, both metadata file-name spellings, a marker mismatch, a
prefixed marker, missing and ambiguous metadata, missing references, duplicate
references and counts. Every archive and every document is generated at run
time by `tests/support/`, whose `meta` module builds documents from freshly
authored values.
[SECURITY.md](../SECURITY.md#threat-model-mapping-metadata-layer) maps every
required XML check to its code prefix and test.

`metadata_evidence.rs` is the independent-evidence layer. It re-expresses the
*structure* of the two official sample documents `profile.md` M9 and M10
describe — an XML declaration, the `ns2` prefix binding, and a reference split
across `ELHELYEZKEDES` and `FAJL_NEV` — using values written from scratch for
this repository. It is evidence of schema shape only: no public sample archive
and no independent conformance corpus exist
([profile.md](profile.md#conformance-evidence)), so agreement with a real
service stays unverified.

## Required future test layers

- CLI integration tests: exact JSON/exit-status contracts, terminal-safe
  human output, sensitive-data-free diagnostics, and bounded stdin/files.
- Filesystem tests: traversal, Unicode/case collisions, existing targets,
  symlinks/reparse points, interrupted writes, and cleanup ownership.
- Writer tests: deterministic bytes for fixed inputs, opaque payload
  preservation, reader compatibility, and independent conformance evidence.
- Fuzz targets: bounded archive inventory and metadata XML parsing; both
  attack surfaces now exist. Retain minimised synthetic regressions.

Use boundary values immediately below, at, and above each limit. Include
contradictory ZIP size declarations, duplicate entries, overlapping records,
compression bombs, unsupported/encrypted members, and archive truncation.
A passing happy-path round-trip is insufficient for safety or conformance.

## Fixture policy

Only independent synthetic originals belong in
[tests/fixtures](../tests/fixtures/README.md). Every committed fixture needs
provenance, its expected outcome, and applicable redistribution terms.
Generate secrets at runtime if a future test needs them; never commit a
private key. Never use real submissions or redacted derivatives.

No private-corpus harness exists. If introduced later, it must be opt-in,
excluded from public CI, and report aggregate counts/error-code buckets
only. No filenames, paths, metadata, payloads, or hashes may enter reports.
