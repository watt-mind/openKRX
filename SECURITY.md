# Security policy

## Current surface

The scaffold supports only help, version, and capability reporting. It does
not parse archives, extract files, use keys, or access government services.
The requirements below are implementation gates for future package support,
not claims that a secure KRX parser already exists.

Only the current `develop` branch receives fixes during scaffolding. There
are no released versions to support yet.

## Reporting

Use [GitHub private vulnerability reporting][report]. Do not publish a
vulnerability as a public issue. Include the affected commit, platform,
expected and actual behavior, and an independently authored synthetic
reproducer. Never send real correspondence or personal metadata.

[report]: https://github.com/watt-mind/openKRX/security/advisories/new

## Required threat model for package support

Assume an attacker controls every archive entry, byte, name, metadata field,
size declaration, and compression choice. Package support must establish:

- Fixed, documented input, entry-count, name-length, metadata, XML-depth,
  node-count, per-entry decoded-size, total decoded-size, and compression
  ratio limits, with checked arithmetic and streaming enforcement.
- No XML DTDs, entity declarations, external resolution, or implicit network
  access. Reject ambiguity rather than selecting a convenient duplicate.
- Detection of conflicting ZIP records, duplicate/colliding paths, unsafe
  names, unsupported methods, encryption, and truncated or corrupt streams.
  ZIP CRC checking concerns corruption, not authenticity.
- Extraction confined to a caller-selected destination, with no traversal,
  absolute paths, symlink/reparse-point escapes, special files, or overwrite.
  Specify Unicode/case collisions and platform-specific residual risks.
- A documented commit/cleanup policy for interrupted writes. Never delete
  pre-existing files or expose partial final output as a successful result.
- Bounded, terminal-safe output. Diagnostics must avoid personal content and
  private paths. Structured metadata is sensitive even when explicitly
  requested; never persist it through telemetry or automatic logging.
- No implicit execution, nested extraction, document rendering, signing,
  certificate verification, or remote submission.

Failure at a required check must prevent the operation from reporting
success. Resource limits must be enforced against actual decoded bytes,
not only attacker-supplied headers. Test these properties before enabling
package operations in the capabilities response.

## Data and key policy

Public fixtures are synthetic originals under their own
[licence](tests/fixtures/LICENSE). Do not commit private files, secrets,
`.env` files, private keys, or complete PEM private-key armour lines.
Do not derive public fixtures from private submissions. Secret-scanner
allowlisting is prohibited. Local private-corpus checks, if explicitly
introduced later, may report only counts and stable error-code buckets.

## Interpretation boundary

Successful parsing, CRC checking, structural validation, or creation is not
cryptographic authentication, proof of delivery, or a legal determination.
An attachment verifier's result applies only to the identified attachment
and checks performed. openKRX must never promote that result to a verdict
on the enclosing package or other payloads.
