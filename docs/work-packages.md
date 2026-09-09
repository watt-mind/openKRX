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

Output: [profile.md](profile.md) holds the evidence table and the list of
unresolved rules, and [conformance.md](conformance.md) maps each rule to
what the code does about it. Treat the unresolved rows as binding
constraints on the packages below, not as open design choices.

## KRX-02: Bounded archive inventory

Status: implemented. `openkrx_core::archive::inventory` reads a bounded,
profile-agnostic inventory from a caller-supplied byte slice. Concrete limits,
supported methods and peak-memory behavior are published in
[architecture.md](architecture.md#limits); the threat-model
mapping is in [SECURITY.md](../SECURITY.md#threat-model-mapping-archive-layer).
The `inventory` fuzz target exists and runs in a bounded CI lane
([testing.md](testing.md#fuzzing)); the truncation and mutation sweeps in
[testing.md](testing.md#sweeps) remain the exhaustive complement to it. No
CLI command exposes the inventory, so `capabilities().operations` remains
empty.

Dependencies: KRX-01's archive-level rules. Owner: core reader contributor;
`crates/openkrx-core/` archive/limits/errors modules and synthetic tests.

Acceptance: typed inventory with documented concrete limits; count actual
decoded bytes; reject ambiguous/conflicting records, unsafe names,
unsupported encryption/methods, truncation, and over-limit input. No implicit
filesystem writes or attachment decoding. Document memory behavior.

Verification: limit boundaries, hostile header/stream disagreements,
duplicate and overlapping entries, compression bombs, and malformed archive
regressions; update the threat-model mapping. The `inventory` fuzz target and
its bounded CI lane exist; a seed corpus and a scheduled long campaign do not.

Must treat as unknown: directory nesting and the location of the `mimetype`
entry, that entry's compression method and byte-exactness, entry-name
character encoding and case rules, and payload directory naming. The
inventory is profile-agnostic: it reports observed entries and never labels
an archive a conforming KRX package.

## KRX-03: Metadata and structural validation

Status: implemented. `openkrx_core::metadata::parse` reads a bounded
`KER_META_V0_9`-shaped document from a caller-supplied byte slice, and
`openkrx_core::profile::check` runs a fixed inventory of eleven structural
checks over an archive inventory and that document. The check inventory
and the metadata limits are published in
[architecture.md](architecture.md#structural-check-inventory), the stable
codes in [codes.md](codes.md); the threat-model
mapping is in
[SECURITY.md](../SECURITY.md#threat-model-mapping-metadata-layer). Each
unresolved rule maps to a distinct `Unresolved(rule)` outcome, so no
conformance verdict exists and the reported summary `Consistent` is explicitly
not one. The `xml_metadata` fuzz target exists and runs in the same
bounded CI lane; the truncation and mutation sweeps in
[testing.md](testing.md#sweeps) remain its exhaustive complement. No CLI
command exposes the checks, so `capabilities().operations` remains empty.

Dependencies: KRX-01 and KRX-02. Owner: profile contributor; core metadata
and validation modules, profile fixtures, and specification updates.

Acceptance: bounded XML without DTD/entity/external resolution, exact
namespace and reference rules, deterministic diagnostics, explicit unknown
profile handling, and a check inventory. Full conformance cannot be claimed
while required rules are unknown. Attachments remain opaque.

Verification: synthetic accepted/rejected metadata, duplicate IDs/references,
missing entries, namespace ambiguity, depth/node/text limits, and an
independent profile example with recorded provenance.

Must treat as unknown: the metadata file name's casing, the unit and
rounding of the declared attachment size, the base path and separator
normalisation of the declared attachment location, and every rule listed as
unresolved in [profile.md](profile.md). Each unknown maps to a distinct
"unsupported or unknown" outcome, never to "invalid", and no full-conformance
verdict may be emitted while any of them stands.

## KRX-04: Reader CLI and stable output

Status: implemented. `openkrx inspect`, `openkrx list` and
`openkrx validate-structure` each take one file or `-`, read at most
`Limits::DEFAULT.max_archive_bytes + 1` bytes, and render
`openkrx_core::archive::inventory` and `openkrx_core::profile::check` without
adding any package semantics. The command contract, the data shapes and the
eight exit statuses are published in
[architecture.md](architecture.md#command-contract-and-json-envelope) and
[Exit statuses](architecture.md#exit-statuses); the two `input.*` codes are in
[codes.md](codes.md). `capabilities().operations` now names exactly those
three commands and the stage is `reader`. `validate-structure` renders every
check as its own outcome and never collapses one into a verdict: `consistent`
means only that nothing failed and nothing was left undecided. No limit
override, no extraction and no write of any kind exists.

Dependencies: KRX-02 and KRX-03, both implemented; the CLI renders
`openkrx_core::archive::inventory` and `openkrx_core::profile::check` and adds
no package semantics of its own. Owner: CLI contributor;
`crates/openkrx-cli/` input/commands/rendering and CLI integration tests.

Acceptance: `inspect`, `list`, and `validate-structure` with bounded input,
one-object JSON, stable errors/exit statuses, explicit profile/check scope,
and terminal-safe output. `validate-structure` renders the check inventory
outcome by outcome and must not collapse it into a `valid` verdict: an
unresolved rule stays visibly distinct from a pass and from a failure.
Capability reporting lists only enabled commands. Update architecture and
README examples. Never report crypto validity.

Verification: subprocess tests for success, malformed/unsupported input,
limits, stdout/stderr separation, stable ordering, and sensitive-data-free
diagnostics. Exercise Linux, macOS, and Windows CI.

## KRX-05: Protected extraction

**Implemented, in two parts.** The planning half is in the core crate:
`openkrx_core::extract::plan` decides what an extraction would create and
refuses everything that could not be created safely, as a pure function of
the archive inventory, with no filesystem access at all. See
[architecture.md](architecture.md#extraction-planning) and the
[extraction planning codes](codes.md#extraction-planning-codes). The
filesystem half is in `crates/openkrx-cli/src/extract/`: a caller-selected
destination that must already exist, no-clobber creation, no link or reparse
point in any path, each planned item decoded through
`ArchiveInventory::entry_bytes`, the `.openkrx-extract.partial` marker and
the undo pass for an interrupted write, and the `extract` command with exit
status 9, the `output.*` codes and its JSON report. See
[architecture.md](architecture.md#extraction-output) and the
[output codes](codes.md#output-codes). `capabilities().operations` names
`extract` now that every property is held by a test.

Dependencies: KRX-02 and KRX-04, both implemented. `extract` joined the
existing command surface: it reuses the bounded input reader, the one-object
JSON envelope, the exit-status categories and the content-free diagnostic
rule of KRX-04, and added the filesystem status and codes that writing needs.
Owner: extraction contributor; core planning and CLI filesystem output
modules, extraction tests, security documentation.

Acceptance: `extract` plans before writes, preserves payload bytes, enforces
aggregate/per-entry limits, never overwrites or escapes the destination, and
has documented interrupted-write cleanup. State platform/race assumptions.
No recursive unpacking, links, special files, or implicit execution.

Verification: traversal/absolute names, case/Unicode collisions, existing
files, parent/leaf symlinks or reparse points, I/O failure injection, and
cleanup without deleting existing data — held by
`crates/openkrx-core/tests/extract_{plan,rejects}.rs` and
`crates/openkrx-cli/tests/extract.rs`. Concurrent replacement is **not**
defended and is stated as a residual risk rather than tested.

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
