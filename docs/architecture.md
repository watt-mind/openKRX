# Architecture and CLI contract

## Current implementation

The edition-2024 Rust workspace targets Rust 1.88 and contains:

| Crate | Responsibility |
| --- | --- |
| `openkrx-core` | Current capability model; future package semantics |
| `openkrx-cli` | `openkrx` executable and human/JSON presentation |

Both crates use `publish = false`. The core crate implements three processing
layers over caller-supplied bytes: `openkrx_core::archive::inventory`, a
bounded, profile-agnostic ZIP container reader; `openkrx_core::metadata::parse`,
bounded schema-shaped parsing of a `KER_META_V0_9` document; and
`openkrx_core::profile::check`, the structural check inventory below. No
extractor or writer is implemented, and no CLI command exposes any of them.
`openkrx --help`, `openkrx --version`, and
`openkrx capabilities [--json]` are the entire supported CLI surface.
Help and version use the argument parser's normal output. Successful
capability reporting exits with status 0; invalid CLI arguments exit with
status 2. Future I/O and format errors need their own reviewed contract.

In JSON mode, capabilities writes exactly one JSON object on stdout:

```json
{
  "schema_version": 1,
  "ok": true,
  "command": "capabilities",
  "data": {
    "project": "openKRX",
    "stage": "scaffold",
    "operations": []
  },
  "verified": false
}
```

`operations` lists implemented package operations; empty means none. An
archive inventory, a metadata parse and a structural report are library
capabilities, not package operations, so the list stays empty until a command
ships. `verified: false` expresses the
cryptographic boundary. Diagnostics belong
on stderr. Consumers should tolerate additional object fields. Incompatible
contract changes require an explicit schema-version decision.

## Planned package architecture

The core crate will accept bounded byte streams or injected readers and
return typed data/errors. It must not open paths, call clocks, launch
processes, or access the network implicitly. The CLI owns bounded file I/O,
argument handling, filesystem protection, and presentation.

Separate ZIP inventory, metadata parsing, profile validation, extraction
planning, and writing. A ZIP inventory can be useful without interpreting a
profile, but it must not be labelled a conforming KRX solely because the
extension or marker matches. Profile rules require cited evidence and must
keep unknown, malformed, and unsupported cases distinguishable.

Proposed commands, not yet available:

| Command | Intended result |
| --- | --- |
| `inspect` | Package/profile inventory and declared metadata |
| `list` | Attachment inventory with stable ordering |
| `validate-structure` | Checks against an explicitly supported profile |
| `extract` | Bounded, no-clobber extraction to a chosen directory |
| `create` | Deterministic package for a documented profile |

Before implementation, each command needs stable error codes, exit-status
categories, limits, privacy rules, and machine-output examples. Avoid bare
`valid` verdicts: specify `valid_structure` and the exact profile/checks.
Unknown required rules must prevent a claim of full conformance.

## Structural check inventory

`openkrx_core::profile::check` runs these checks, in this order, and reports
each as `Pass`, `Fail(code)`, `Unresolved(rule)` or `NotApplicable`. Rule ids
are those of [profile.md](profile.md). An unresolved rule never becomes a
failure and never becomes a pass.

| # | Check | Rules | Fail codes | Unresolved as |
| --- | --- | --- | --- | --- |
| 1 | `metadata_location` | A4 | `metadata.missing`, `metadata.ambiguous.multiple_candidates` | — |
| 2 | `metadata_file_name` | M12 | — | M12 |
| 3 | `root_prefix` | A19 | — | A19 |
| 4 | `marker_entry` | A2 | `metadata.malformed.marker_missing`, `.marker_not_first`, `.marker_content` | A19 |
| 5 | `metadata_parse` | M1–M8 | every `metadata.unsupported.*`, `metadata.malformed.*` and `metadata.over_limit.*` code | — |
| 6 | `schema_optional_fields` | M11 | — | M11 |
| 7 | `handling_instructions_form` | M8 | — | — |
| 8 | `attachment_references` | M5, M10, M14 | `metadata.reference.missing_entry` | M14 |
| 9 | `attachment_count` | M7 | `metadata.count_mismatch` | — |
| 10 | `attachment_uniqueness` | A6 | `metadata.reference.duplicate` | — |
| 11 | `declared_size` | M13 | — | M13 |

A candidate metadata entry is any entry whose last path segment equals
`KULDEMENY_META.xml` and whose parent segment equals `Metalayer`, both compared
ASCII-case-insensitively, with any number of leading segments. That shape is
observation, not assumption: A19 leaves the nesting open and M12 leaves the
casing open, so the observed root prefix and file name are reported as facts
and the corresponding checks stay unresolved rather than deciding either way.
A20 — the marker entry's compression method and byte-exactness — is never
asserted at all. `MERET` is parsed but never compared to an entry's size;
check 11 reports both values side by side and stays unresolved under M13.

The report's summary is `Consistent`, `Inconsistent` or `Unresolved`.
**`Consistent` is not a conformance claim.** It means only that no check
failed and none was left unresolved. openKRX cannot claim structural
conformance while [profile.md](profile.md#unresolved-essential-rules) lists
unresolved essential rules, and a package that is internally consistent may
still be refused by a real service (M15). There is deliberately no `valid`,
`conforming` or `is_krx` field in the API.

## Safety and determinism

All archive and XML boundaries in [SECURITY.md](../SECURITY.md) are required
before package features ship. The archive and metadata layers implement their
share of them. Extraction and writer limits are still unimplemented and
unlisted.

### Archive inventory limits

`Limits::DEFAULT` carries these values. Every field is public, so a caller may
tighten any of them; the defaults are the contract, and changing one requires
changing this table in the same commit with a written rationale.

| Limit | Default | Enforced against |
| --- | --- | --- |
| `max_archive_bytes` | 64 MiB | length of the input slice |
| `max_entries` | 256 | count declared by the end record, before the directory is walked |
| `max_name_bytes` | 255 | each entry name |
| `max_entry_decoded_bytes` | 32 MiB | bytes actually decoded for one entry |
| `max_total_decoded_bytes` | 128 MiB | bytes actually decoded across all entries |
| `max_compression_ratio` | 100 | decoded divided by compressed, once an entry passes 64 KiB |
| `max_extra_field_bytes` | 4 KiB | each local and each central extra-field block |
| `max_comment_bytes` | 1 KiB | the archive comment and each entry comment |

The 64 KiB ratio grace window exists because a small entry can compress badly
for legitimate reasons; below that volume the per-entry ceiling already bounds
the work. The remaining defaults are sized for the packages the format
description implies — a marker entry, one metadata document, and a small
number of attachments (`profile.md` rules A5 and A10) — and are deliberately
far below what a general-purpose ZIP reader would accept.

### Metadata limits

`MetadataLimits::DEFAULT` carries these values. Every field is public, so a
caller may tighten any of them; the defaults are the contract, and changing one
requires changing this table in the same commit with a written rationale.

| Limit | Default | Enforced against |
| --- | --- | --- |
| `max_document_bytes` | 32 MiB | length of the document slice, before parsing starts |
| `max_depth` | 32 | element nesting, counting the root as depth 1 |
| `max_elements` | 10 000 | element start events produced |
| `max_attributes_per_element` | 32 | attributes on one element |
| `max_text_bytes` | 1 MiB | character data produced across the whole document |

Every limit is counted inside the XML event loop against values actually
produced, never against a declaration inside the document, and all arithmetic
is checked or saturating. `max_document_bytes` matches
`max_entry_decoded_bytes` because the document reaches the parser through
`ArchiveInventory::entry_bytes` and cannot be larger than that ceiling. The
grammar `KER_META_V0_9` describes needs six levels of nesting and a few dozen
elements, so the defaults are far above a real document and far below what a
general-purpose XML reader would accept.

A `<!DOCTYPE ...>` declaration, a general entity reference other than the five
XML predefines, a processing instruction other than the XML declaration, and a
declared encoding other than UTF-8 are each refused with their own
`metadata.unsupported.*` or `metadata.malformed.*` code rather than being
processed. No entity is ever declared, so none is ever resolved; `quick-xml`
opens no stream of its own, and the core crate has no filesystem, clock,
process or network access for a resolved entity to reach.

Supported compression methods are 0 (stored) and 8 (deflate). ZIP64 records,
markers and extra fields, encryption and strong-encryption flags, patched
data, multi-disk archives and every other method are refused with an
`archive.unsupported.*` code rather than being decoded or ignored.

### Peak memory and streaming

Peak additional memory for an inventory is the input slice the caller already
holds, plus one `miniz_oxide` inflate state, plus one 64 KiB output buffer,
plus per-entry metadata (names borrow the input slice; nothing is copied).
Decoded payloads are counted, CRC-checked and discarded, never retained, so
memory does not grow with decoded volume. Because the output buffer is the
enforcement granularity, no limit is ever exceeded by more than one buffer
before decoding aborts. `ArchiveInventory::entry_bytes` is the one exception
and is opt-in: it decodes a single named entry into a `Vec<u8>` bounded by
`max_entry_decoded_bytes`, for structural documents, not attachments.

Ambiguity is rejected, never resolved. More than one end-of-central-directory
record that terminates the image, an entry name that collides with another
byte-exactly or after case folding, and any byte the declared structures do
not exactly cover are all errors, because each would let two readers disagree
about the same archive. Entry-name case rules are unresolved (`profile.md`
A21), which is precisely why a case-folded collision cannot be resolved here.

The writer must use caller-supplied timestamps and deterministic ordering,
ZIP settings, names, and XML serialization. Equal inputs and settings must
produce equal bytes. Creation must enforce the reader's applicable limits
and preserve opaque attachment bytes exactly. No implicit signing occurs.

Filesystem behavior belongs to a reviewed output layer. Specify no-clobber
publication, rollback, symlink/reparse-point defense, and race assumptions
for each supported OS; do not copy ZIP entry names directly into file paths.

## Integration boundaries

openSzigno may produce or verify an `.es3` attachment. openKRX preserves its
bytes without interpreting its signatures. Calling an external verifier is
an explicit consumer action; its result cannot authenticate the package.

openPapir will consume package data for local cases and receipt association.
Case storage, workflow state, receipt authenticity, government credentials,
and delivery integrations stay outside openKRX. Creating a profile-shaped
archive does not establish that a government service accepts uploads of it.
