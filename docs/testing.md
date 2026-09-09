# Testing and quality gates

## Foundation checks

Run `bash scripts/check.sh` locally. CI workflow files define the exact
commands for formatting, Clippy, Rust documentation, Markdown/local links,
file length, tests on Linux/macOS/Windows, MSRV, cargo-deny, cargo-machete,
coverage, and security scanning. The initial workspace line-coverage floor
is 90%. Crates are unpublished and tests use the committed lockfile.

Current tests cover the capability model and executable contract, including
JSON shape, help/version, and argument rejection, plus the bounded archive
inventory. They provide no evidence about XML safety, extraction, or profile
conformance, because those operations do not exist yet.

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

## Required future test layers

- Core unit tests: malformed XML, profile rules, and metadata ambiguity.
- CLI integration tests: exact JSON/exit-status contracts, terminal-safe
  human output, sensitive-data-free diagnostics, and bounded stdin/files.
- Filesystem tests: traversal, Unicode/case collisions, existing targets,
  symlinks/reparse points, interrupted writes, and cleanup ownership.
- Writer tests: deterministic bytes for fixed inputs, opaque payload
  preservation, reader compatibility, and independent conformance evidence.
- Fuzz targets: bounded archive inventory and XML parsing as soon as these
  attack surfaces exist; retain minimised synthetic regressions.

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
