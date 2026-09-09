# Testing and quality gates

## Foundation checks

Run `bash scripts/check.sh` locally. CI workflow files define the exact
commands for formatting, Clippy, Rust documentation, Markdown/local links,
file length, tests on Linux/macOS/Windows, MSRV, cargo-deny, cargo-machete,
coverage, and security scanning. The initial workspace line-coverage floor
is 90%. Crates are unpublished and tests use the committed lockfile.

Current tests cover the capability model and executable contract, including
JSON shape, help/version, and argument rejection. They provide no evidence
about archive or XML safety because those operations do not exist yet.

## Required future test layers

- Core unit tests: bounded ZIP inventory, actual decoded byte accounting,
  malformed XML, profile rules, ambiguity, and checked arithmetic.
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
