# Architecture and CLI contract

This is the canonical reference for what openKRX does today. Every claim
here describes code that exists on the default branch. Where a behaviour is
planned, the section says so in as many words. Nothing in this document,
and nothing the code emits, states that an archive is a conforming KRX
package: [profile.md](profile.md#unresolved-essential-rules) lists essential
rules that no primary source settles, so that claim is not available.

## Goals

- **Bounded processing of hostile input.** Every archive and every metadata
  document is treated as attacker-controlled. Limits are enforced against
  bytes actually produced, never against a size a header declares.
- **Ambiguity is rejected, not resolved.** Where two readers could disagree
  about the same bytes, openKRX refuses the input instead of choosing.
- **Observation, not verdict.** The library reports what it read. It has no
  `valid`, `conforming` or `is_krx` field, and an unresolved format rule is
  reported as unresolved rather than folded into a pass or a failure.
- **Content-free diagnostics.** A diagnostic carries a stable dotted code,
  numeric limit values and an entry index. It never carries an entry name,
  entry bytes, element text or attribute values.
- **No implicit I/O.** The core crate performs no filesystem, clock, process
  or network access, and resolves nothing external. Byte slices go in,
  typed values or typed errors come out.
- **Determinism.** The same bytes and the same limits produce the same
  inventory, the same parse and the same check outcomes.

## Not yet implemented

The library layers below are reachable from Rust only. No command exposes
them, and `capabilities().operations` is therefore empty.

- Every reader command: `inspect`, `list` and `validate-structure`. Their
  contracts are specified in [work-packages.md](work-packages.md), not here.
- Extraction of any kind. Nothing writes to a filesystem, so no no-clobber
  rule, path sanitisation policy, symlink defence or cleanup policy exists
  yet; those are requirements in [SECURITY.md](../SECURITY.md), not
  implemented behaviour.
- Package creation, deterministic writing and any writer limits.
- Signature handling of any kind, including `signatures.xml` (rule A7).
  `signatures.xml` is an ordinary entry to the inventory and nothing else.
- Attachment interpretation. Attachments are named and, when a reference
  resolves, their decoded size is counted; their bytes are never decoded
  into memory by the profile layer and never parsed.
- ZIP64, encryption, multi-disk archives and every compression method other
  than stored and deflate. These are refused, not deferred; see
  [Parser safety model](#parser-safety-model).
- Fuzz targets. Exhaustive truncation and single-byte mutation sweeps stand
  in for them; see [testing.md](testing.md#fuzzing-planned).

## Crate shape

```text
crates/
  openkrx-core/   # bounded ZIP inventory, metadata parsing, structural checks
  openkrx-cli/    # argument parsing, human and JSON presentation
```

Both crates set `publish = false`. The workspace uses edition 2024 and
targets Rust 1.88 or newer.

`openkrx-core` holds all format semantics and performs no I/O. Its only
dependencies are `serde`, `miniz_oxide` and `quick-xml`; it deliberately does
not use a general-purpose ZIP crate, because the strictness rules below are
exactly the decisions such a crate would make differently. `openkrx-cli` owns
argument handling and presentation and carries no package semantics.

The error, check-outcome and rule-identifier enums are `#[non_exhaustive]`.
A consumer matches on the stable dotted code, and must treat an unknown code
as a failure rather than as a success.

### Module map: `openkrx-core`

| File | Responsibility |
| --- | --- |
| `src/lib.rs` | Crate root: re-exports, and `capabilities()`, which reports the implementation stage and an empty operation list. |
| `src/error.rs` | `ArchiveError` and its six category enums, each mapping to a stable dotted code; `Display` prints code, entry index and numbers only. |
| `src/limits.rs` | `Limits`, the archive ceilings, with `DEFAULT`, the ratio grace window and the inflate output-buffer size. |
| `src/archive/mod.rs` | `inventory()`, the entry point: assembles the inventory, enforces exact byte coverage, and exposes `ArchiveEntry` and `entry_bytes`. |
| `src/archive/eocd.rs` | Locates the end-of-central-directory record; two terminating candidates are an ambiguity, not a choice. |
| `src/archive/central.rs` | Parses central-directory records and validates features, name safety, collisions and declared limits before any data is read. |
| `src/archive/local.rs` | Checks each local file header, and each data descriptor, against its authoritative central-directory record. |
| `src/archive/inflate.rs` | Bounded stored/deflate decoding with streaming limit enforcement and CRC-32 checking (corruption, not authenticity). |
| `src/archive/names.rs` | Entry-name safety classes and collision rules, applied to raw bytes before any encoding decision. |
| `src/archive/raw.rs` | Checked little-endian reads over the borrowed image; every read reports truncation at a named structure. |
| `src/metadata/mod.rs` | `parse()`, the metadata entry point, and the module's public re-exports. |
| `src/metadata/error.rs` | `MetadataError` and its three category enums; `Display` prints the code and limit numbers only, never document content. |
| `src/metadata/limits.rs` | `MetadataLimits`, the XML ceilings, with `DEFAULT`. |
| `src/metadata/scanner.rs` | Bounded XML event source over `quick-xml`: refuses DTDs, non-predefined entities and stray processing instructions, and counts depth, elements, attributes and text while streaming. |
| `src/metadata/reader.rs` | The `KER_META_V0_9` grammar (rules M1 to M8) re-expressed as code, with the order rule M2 confines and the two leniencies M11 forces. |
| `src/metadata/model.rs` | Typed shape of a parsed document; open values such as `MERET` keep their verbatim text beside an optional numeric reading. |
| `src/metadata/field.rs` | `MetadataField`, the schema-fixed element identities an error may name without disclosing content. |
| `src/profile/mod.rs` | `check()`, the fixed check inventory, the `codes` module of structural failure codes, and `ProfileError`. |
| `src/profile/locate.rs` | Finds the metadata candidate and the format marker by shape, reporting the observed prefix and casing as facts. |
| `src/profile/references.rs` | Resolves `ELHELYEZKEDES` joined to `FAJL_NEV` against real entry names, byte-exactly or as a prefix variant. |
| `src/profile/report.rs` | `CheckId`, `CheckOutcome`, `RuleId`, `StructureReport`, `StructureSummary` and the observation and attachment-resolution types. |

### Module map: `openkrx-cli`

| File | Responsibility |
| --- | --- |
| `src/main.rs` | The whole executable: the `clap` parser, the `capabilities` subcommand, the human line pair, and the one-object JSON response. |

## Format scope

KRX, as this project uses the word, means the ZIP-based document container
described by the primary sources registered in
[references.md](references.md) and reduced to numbered rules in
[profile.md](profile.md). It is the Hungarian realisation of the SPOCS OCD
container: a marker entry naming the format, a `Metalayer` directory holding
one descriptive metadata document, and payload subdirectories holding one
opaque attachment each.

Three scopes exist in the code, and they are deliberately separate.

| Layer | Scope |
| --- | --- |
| `archive::inventory` | Profile-agnostic. It reads any ZIP image inside its limits and knows nothing about KRX at all. Nothing in it inspects a name for a `Metalayer` segment or a `mimetype` marker. |
| `metadata::parse` | Grammar-shaped. It reads one XML document against the shape rules M1 to M8 of `KER_META_V0_9` describe. It is not a schema validator, no XSD is vendored, and it never sees the archive. |
| `profile::check` | The only layer that relates the two. It runs the fixed check inventory over an inventory and the document it holds. |

The metadata target namespace is `http://xsd.orfk.hu/rzs/ker/kuldemeny` and
the only profile version token any source publishes is `KRX_VERZIOSZAM`
`v0.9`. The archive layout carries no version marker at all, so an
archive-level version cannot be detected without reading the metadata.

What is out of scope in every layer: attachment content of any kind,
`signatures.xml` content, `DeliveryInstruction.xml`, `message.properties`,
`ControlMessage.xml` and envelope images. Those entries are inventoried by
name like any other and are never opened.

## Parser safety model

Four rules carry the safety argument. Each is a property of the code, and
[SECURITY.md](../SECURITY.md#threat-model-mapping-archive-layer) maps each
required check to the stable code and test that hold it.

**Exact coverage.** Every byte of the archive image must be claimed by
exactly one declared structure: a local header, its entry data, an optional
data descriptor, a central-directory record, or the end record and its
comment. Bytes before the first local header, bytes after the end record's
comment, bytes claimed by nothing, and two claimed ranges that overlap are
each a distinct `archive.malformed.*` failure. A general-purpose reader
tolerates all four; openKRX does not, because each is a place where two
readers can be made to see different archives.

**Ambiguity is rejected, never resolved.** More than one end-of-central-
directory record that terminates the image is refused rather than settled by
"take the last one". Two entry names that are byte-identical, or that differ
only by case, are refused rather than deduplicated. Case folding is treated
as a collision precisely because rule A21 leaves entry-name case rules
unresolved: with no rule to appeal to, either choice would be an invention.
The central directory is authoritative, and a local header or data
descriptor that contradicts its record is a failure rather than a fallback.

**XML features are refused at the event boundary.** A `<!DOCTYPE ...>`
declaration is refused before anything can be declared, so no entity is ever
declared and no external identifier is ever seen. A general entity reference
other than the five XML predefines is refused, which is the first thing an
external-entity attack must produce. A processing instruction other than the
XML declaration is refused. A declared encoding other than UTF-8 is refused
rather than guessed at. `quick-xml` opens no stream of its own, and the core
crate has no filesystem, clock, process or network access, so a resolved
entity would have nothing to reach even in principle.

**Streaming byte accounting.** Archive limits are checked against bytes the
decoder actually produced, not against the uncompressed size a header
declares, and metadata limits are counted inside the XML event loop against
events actually produced. All arithmetic is checked or saturating.

### Peak memory and streaming

Peak additional memory for an inventory is the input slice the caller
already holds, plus one `miniz_oxide` inflate state, plus one 64 KiB output
buffer, plus per-entry metadata; names borrow the input slice and nothing is
copied. Decoded payloads are counted, CRC-checked and discarded, never
retained, so memory does not grow with decoded volume. Because the output
buffer is the enforcement granularity, no limit is ever exceeded by more
than one buffer before decoding aborts.

`ArchiveInventory::entry_bytes` is the one exception and is opt-in: it
decodes a single named entry into a `Vec<u8>` bounded by
`max_entry_decoded_bytes`. The profile layer calls it for the marker entry
and the metadata document only. Attachments are never decoded into memory.

Supported compression methods are 0 (stored) and 8 (deflate). ZIP64 records,
markers and extra fields, encryption and strong-encryption flags, patched
data, multi-disk archives and every other method are refused with an
`archive.unsupported.*` code rather than being decoded or ignored.

## Limits

Both limit structs are `Copy`, carry no interior state, and expose every
field publicly so a caller may tighten (or, deliberately, relax) a single
bound. The defaults below are the contract: changing one requires changing
the table in the same commit, with a written rationale.

### Archive inventory limits

`Limits::DEFAULT` carries these values.

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

Two constants support the table. `Limits::RATIO_GRACE_BYTES` is 64 KiB: the
decoded volume an entry may produce before the ratio check starts, because a
small entry can compress badly for legitimate reasons and the per-entry
ceiling already bounds the work below that volume.
`Limits::OUTPUT_BUFFER_BYTES` is also 64 KiB and is the enforcement
granularity described above.

The remaining defaults are sized for the packages the format description
implies — a marker entry, one metadata document, and a small number of
attachments (rules A5 and A10) — and are deliberately far below what a
general-purpose ZIP reader would accept.

### Metadata limits

`MetadataLimits::DEFAULT` carries these values.

| Limit | Default | Enforced against |
| --- | --- | --- |
| `max_document_bytes` | 32 MiB | length of the document slice, before parsing starts |
| `max_depth` | 32 | element nesting, counting the root as depth 1 |
| `max_elements` | 10 000 | element start events produced |
| `max_attributes_per_element` | 32 | attributes on one element |
| `max_text_bytes` | 1 MiB | character data produced across the whole document |

`max_document_bytes` matches `max_entry_decoded_bytes` because the document
reaches the parser through `ArchiveInventory::entry_bytes` and cannot be
larger than that ceiling. The grammar `KER_META_V0_9` describes needs six
levels of nesting and a few dozen elements, so the defaults are far above a
real document and far below what a general-purpose XML reader would accept.

## Structural check inventory

`openkrx_core::profile::check` runs these eleven checks, in this order —
`CheckId::ORDER` — and reports each as `Pass`, `Fail(code)`,
`Unresolved(rule)` or `NotApplicable`. Rule ids are those of
[profile.md](profile.md); the full rule-to-check map, including the rules no
check asserts, is in [conformance.md](conformance.md). An unresolved rule
never becomes a failure and never becomes a pass.

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

Six of the eleven can report `Unresolved`, citing exactly five rules: M12
(check 2), A19 (checks 3 and 4), M11 (check 6), M14 (check 8) and M13
(check 11). No other check can. A check whose input is absent reports
`NotApplicable`: checks 7 to 11 do so when the document declares no
dispatch, no attachment or no count, or when none of its dispatches carries
an unqualified handling instruction; and every check after the first — except
check 4 `marker_entry`, which is decided from the entry names alone and runs
regardless — reports `NotApplicable` when no metadata document could be
located.

A candidate metadata entry is any entry whose last path segment equals
`KULDEMENY_META.xml` and whose parent segment equals `Metalayer`, both
compared ASCII-case-insensitively, with any number of leading segments.
That shape is observation, not assumption: A19 leaves the nesting open and
M12 leaves the casing open, so the observed root prefix and file name are
reported as facts and the corresponding checks stay unresolved rather than
deciding either way. A20 — the marker entry's compression method and
byte-exactness — is never asserted at all. `MERET` is parsed but never
compared to an entry's size; check 11 reports both values side by side and
stays unresolved under M13.

Beside the outcomes, the report carries `Observations` (the observed root
prefix, the metadata entry's name and index, `KRX_VERZIOSZAM`,
`FORRASRENDSZER_AZONOSITO`, `KULDEMENY_TIPUS` and the listed attachment
count) and one `AttachmentResolution` per declared attachment, each holding
the declared path, how it resolved, the declared size as written, its
numeric reading when it has one, and the observed decoded size.

The report's summary is `Consistent`, `Inconsistent` or `Unresolved`.
**`Consistent` is not a conformance claim.** It means only that no check
failed and none was left unresolved. openKRX cannot claim structural
conformance while [profile.md](profile.md#unresolved-essential-rules) lists
unresolved essential rules, and a package that is internally consistent may
still be refused by a real service (M15). There is deliberately no `valid`,
`conforming` or `is_krx` field in the API.

## Command contract and JSON envelope

`openkrx --help`, `openkrx --version` and `openkrx capabilities [--json]`
are the entire supported CLI surface. Help and version use the argument
parser's normal output. No other command exists, and none of the three
library layers is reachable from the command line.

In JSON mode, `capabilities` writes exactly one JSON object on stdout:

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

Without `--json` the same command prints two human lines: the project name
and stage, then a sentence stating that no document operation is implemented
and nothing is verified. Diagnostics belong on stderr; stdout carries the
response and nothing else.

`operations` lists implemented package operations, and empty means none. An
archive inventory, a metadata parse and a structural report are library
capabilities, not package operations, so the list stays empty until a
command ships. `verified: false` expresses the cryptographic boundary and is
never `true` in this design.

**The `schema_version` compatibility rule.** `schema_version` is `1`. A
consumer must ignore object fields it does not recognise, because adding a
field is not a breaking change and will not raise the version. A field being
removed, renamed, or given a different type or meaning is a breaking change,
and requires an explicit decision to raise `schema_version` in the same
change that makes it. A consumer that reads an unknown `schema_version`
must stop rather than guess.

Reader commands are specified, not implemented. Their contracts — bounded
input, the one-object rule, stable error codes, exit-status categories and
the requirement that `validate-structure` render the check inventory
outcome by outcome rather than collapse it into a verdict — are in
[work-packages.md](work-packages.md), package KRX-04. Until such a command
ships, this section describes the complete CLI.

## Exit statuses

Only two statuses exist today, because only one command exists.

| Status | Meaning |
| --- | --- |
| 0 | The command succeeded. Today this means capability reporting completed. |
| 2 | The arguments were rejected: an unknown or missing subcommand, or an invalid flag. This is the argument parser's status. |

There is no exit status for a format error, an over-limit input or an I/O
failure, because no command can encounter one. A reader command must define
its own status categories before it ships, and adding one is a contract
change that belongs in the changelog.

## Stable codes

Every failure this crate produces carries a stable dotted code. The archive
layer produces `archive.*`, the metadata layer and the profile layer both
produce `metadata.*`, and the two `metadata.*` sets are disjoint, so a
consumer can bucket every diagnostic by its dotted code alone.

The complete catalogue — every code, its category, its meaning, the numeric
fields its error carries, and the test that asserts it — is
[codes.md](codes.md). That document is kept honest by
`scripts/check-codes.py`, which fails when a code exists in the sources but
not in the catalogue, or the other way round.

A code is part of the public contract. Renaming one, or changing which
condition produces it, is a breaking change. Adding a new code is not, which
is why every error enum is `#[non_exhaustive]` and a consumer must treat an
unknown code as a failure.

## Boundaries

**Cryptographic boundary.** openKRX performs no cryptography. It verifies no
signature, checks no certificate, and computes no digest for any purpose
other than the ZIP CRC-32, which detects corruption and says nothing about
authenticity. `verified: false` in the JSON envelope states this. A
successful inventory, parse or structural report is not authentication, not
proof of delivery and not a legal determination.

**Interpretation boundary.** The library reports what the bytes say. It does
not interpret a declared timestamp (it has no clock and keeps
`KULDEMENY_LETREHOZASANAK_IDEJE` in its lexical form), does not convert a
declared size (rule M13 leaves the unit open), does not decide what a
receiving service requires beyond the schema (rule M15), and does not open
an attachment. Where a source leaves a rule open, the outcome is
`Unresolved` citing that rule.

**Integration with openSzigno.** [openSzigno][openszigno] produces and
verifies Microsec `.es3` dossiers. An `.es3` file inside a KRX package is an
opaque attachment here: openKRX preserves its bytes and never interprets its
signatures. Calling openSzigno on an extracted attachment is an explicit
consumer action, and its result concerns the dossier it checked. It cannot
be promoted to a statement about the enclosing package or any other payload.

**Integration with openPapir.** [openPapir][openpapir] is the planned local
correspondence workflow consumer: cases, imported packages and receipts.
Case storage, workflow state, receipt authenticity, government credentials
and delivery integrations stay outside openKRX. openKRX owns the container
layer, preserves attachment bytes and leaves interpretation to explicit
downstream tools. Producing a profile-shaped archive would not establish
that a government service accepts uploads of it.

[openszigno]: https://github.com/watt-mind/openSzigno
[openpapir]: https://github.com/watt-mind/openPapir
