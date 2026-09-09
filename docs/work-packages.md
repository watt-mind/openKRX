# Implementation work packages

These are specification identifiers and implementation sequencing, not a
live backlog or external issue-tracker IDs. The maintainer execution queue
lives in the configured issue tracker; public GitHub issues provide intake.
Convert one package into a scoped issue before dispatch. Its issue needs
owned paths, prerequisites, non-goals, acceptance criteria, verification,
and a link to the applicable evidence.

## KRX-01: Establish the first profile

Dependencies: none. Owner: format-research contributor; `docs/references.md`
and a new profile specification under `docs/`.

Acceptance: identify a supported profile/version, required entries,
namespaces, metadata fields, attachment references, and ambiguous rules with
section-level primary citations. Record redistribution terms before adding
any third-party material. Separate unknowns from requirements and identify
independent synthetic conformance evidence. No parser claims yet.

Verification: another contributor can trace each normative rule to its
source; documentation checks pass. Unresolved essential rules block the
associated conformance/writer work, not generic bounded ZIP research.

## KRX-02: Bounded archive inventory

Dependencies: KRX-01's archive-level rules. Owner: core reader contributor;
`crates/openkrx-core/` archive/limits/errors modules and synthetic tests.

Acceptance: typed inventory with documented concrete limits; count actual
decoded bytes; reject ambiguous/conflicting records, unsafe names,
unsupported encryption/methods, truncation, and over-limit input. No implicit
filesystem writes or attachment decoding. Document memory behavior.

Verification: limit boundaries, hostile header/stream disagreements,
duplicate and overlapping entries, compression bombs, and malformed archive
regressions; add a fuzz target and update the threat-model mapping.

## KRX-03: Metadata and structural validation

Dependencies: KRX-01 and KRX-02. Owner: profile contributor; core metadata
and validation modules, profile fixtures, and specification updates.

Acceptance: bounded XML without DTD/entity/external resolution, exact
namespace and reference rules, deterministic diagnostics, explicit unknown
profile handling, and a check inventory. Full conformance cannot be claimed
while required rules are unknown. Attachments remain opaque.

Verification: synthetic accepted/rejected metadata, duplicate IDs/references,
missing entries, namespace ambiguity, depth/node/text limits, and an
independent profile example with recorded provenance.

## KRX-04: Reader CLI and stable output

Dependencies: KRX-02 and KRX-03. Owner: CLI contributor;
`crates/openkrx-cli/` input/commands/rendering and CLI integration tests.

Acceptance: `inspect`, `list`, and `validate-structure` with bounded input,
one-object JSON, stable errors/exit statuses, explicit profile/check scope,
and terminal-safe output. Capability reporting lists only enabled commands.
Update architecture and README examples. Never report crypto validity.

Verification: subprocess tests for success, malformed/unsupported input,
limits, stdout/stderr separation, stable ordering, and sensitive-data-free
diagnostics. Exercise Linux, macOS, and Windows CI.

## KRX-05: Protected extraction

Dependencies: KRX-02 and KRX-04. Owner: extraction contributor; core planning
and CLI filesystem output modules, extraction tests, security documentation.

Acceptance: `extract` plans before writes, preserves payload bytes, enforces
aggregate/per-entry limits, never overwrites or escapes the destination, and
has documented interrupted-write cleanup. State platform/race assumptions.
No recursive unpacking, links, special files, or implicit execution.

Verification: traversal/absolute names, case/Unicode collisions, existing
files, parent/leaf symlinks or reparse points, concurrent replacement where
defended, I/O failure injection, and cleanup without deleting existing data.

## KRX-06: Deterministic profile writer

Dependencies: KRX-01, KRX-03, and KRX-05's safe output layer. Owner: writer
contributor; core authoring modules, CLI create, writer tests and contract.

Acceptance: `create` accepts an explicit supported profile and metadata,
uses caller-supplied time, preserves attachment bytes, validates references
and limits, and produces byte-identical output for identical inputs. Refuse
existing outputs. No signing, encryption, or submission functionality.

Verification: determinism across repeated runs, reader round-trips,
independent profile checks, invalid metadata, size overflow, payload byte
identity, and output failure/no-clobber cases on all supported OSes.

## KRX-07: Consumer contract and first release review

Dependencies: KRX-04, KRX-05, and KRX-06. Owner: integration contributor;
consumer examples, public contract documentation, and release checklist.

Acceptance: synthetic openPapir import integration with explicit version
compatibility; document openSzigno attachment handoff and exact verification
scope. Review first-release readiness separately. No hidden sibling-path or
network dependency in either project's build.

Verification: independent consumer contract tests and a clean-checkout
build; all [release gates](releasing.md) have recorded outcomes.

## Dispatch policy

KRX-01 is first. After the reader core is ready, CLI presentation and future
extraction planning can be split only with explicit module ownership and
agreed interfaces. Keep one issue/branch per bounded change. File newfound
profile gaps and safety follow-ups separately; do not relax a check to
increase compatibility. Security findings use the private reporting flow.
