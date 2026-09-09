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

The three library layers are reachable from the command line: `inspect`,
`list` and `validate-structure` render them, `extract` writes what
[Extraction planning](#extraction-planning) decided, and
`capabilities().operations` names exactly those four. Everything below is
still absent.

- Package creation, deterministic writing and any writer limits. `extract`
  writes files out of a package; nothing writes a package.
- Atomic whole-tree extraction. Files are created directly in the
  destination, not staged elsewhere and renamed into place, so an interrupted
  run is detected rather than prevented; see
  [Extraction output](#extraction-output).
- `openat2`-style path resolution. Confinement is enforced with
  `symlink_metadata` and exclusive creation, which defend against what is
  already at the destination and not against a concurrent writer with access
  to it. The assumption is stated under
  [Race assumptions](#race-assumptions-and-what-they-do-not-cover).
- Signature handling of any kind, including `signatures.xml` (rule A7).
  `signatures.xml` is an ordinary entry to the inventory and nothing else.
- Attachment interpretation. Attachments are named and, when a reference
  resolves, their decoded size is counted; their bytes are never decoded
  into memory by the profile layer and never parsed.
- ZIP64, encryption, multi-disk archives and every compression method other
  than stored and deflate. These are refused, not deferred; see
  [Parser safety model](#parser-safety-model).
- Fuzzing beyond a smoke lane. Both readers have a `cargo-fuzz` target,
  `inventory` and `xml_metadata`, and CI runs each for 30 seconds per push —
  a smoke budget that proves the harness executes, not a campaign. There is
  no seed corpus, no scheduled long run, and no target for `profile::check`
  or `extract::plan`. The exhaustive truncation and single-byte mutation
  sweeps remain the compensating control; see
  [testing.md](testing.md#fuzzing).

## Crate shape

```text
crates/
  openkrx-core/   # bounded ZIP inventory, metadata parsing, structural checks
  openkrx-cli/    # argument parsing, human and JSON presentation
```

Both crates set `publish = false`. The workspace uses edition 2024 and
targets Rust 1.88 or newer.

`openkrx-core` holds all format semantics and performs no I/O. Its only
dependencies are `serde`, `miniz_oxide`, `quick-xml` and
`unicode-normalization` (MIT OR Apache-2.0), the last of which supplies the
NFC normalisation the extraction planner compares destination paths with. It
deliberately does not use a general-purpose ZIP crate, because the
strictness rules below are exactly the decisions such a crate would make
differently. `openkrx-cli` owns
argument handling and presentation and carries no package semantics.

`openkrx-core` has one feature, `synthetic-writer`, which is off by default
and compiles the test-only synthetic writers in `src/synthetic/`. It exists so
that the command-line crate's subprocess tests can build the same archives the
core tests build, without a duplicate writer and without a committed binary
fixture. Both crates enable it as a development dependency only; `cargo build`
and `cargo build --release` never compile it into a binary.

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
| `src/archive/kind.rs` | `EntryKind` and the host-system mapping from `version made by` and `external file attributes` onto regular file, directory, symlink, special file or unknown. |
| `src/archive/raw.rs` | Checked little-endian reads over the borrowed image; every read reports truncation at a named structure. |
| `src/extract/mod.rs` | `plan()`, the extraction-planning entry point: `ExtractLimits`, `ExtractionPlan` and `PlanItem`, and the per-entry walk that produces them. |
| `src/extract/error.rs` | `PlanError` and its four category enums, each mapping to a stable dotted code; `Display` prints code, entry index and numbers only. |
| `src/extract/paths.rs` | Destination path components and the shapes that must never become one, checked against the union of the three target platforms' rules. |
| `src/extract/collisions.rs` | NFC and case-folded path collisions, file-versus-directory conflicts, and the deduplicated implicit parent directories. |
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
| `src/synthetic/mod.rs` | Test-only synthetic ZIP writer, behind the non-default `synthetic-writer` feature; it can emit contradictory headers on purpose so hostile archives are built exactly. No released binary contains it. |
| `src/synthetic/meta.rs` | Test-only synthetic metadata documents and KRX-shaped archives, built from values authored for this repository. |

### Module map: `openkrx-cli`

| File | Responsibility |
| --- | --- |
| `src/main.rs` | The `clap` parser and dispatch, and nothing else: it reads the input, calls the two core entry points, and hands the result to a renderer. |
| `src/input.rs` | The only I/O in openKRX: opening a file exactly as named, or reading standard input as binary, bounded by the input cap. |
| `src/skill.rs` | The agent skill document, embedded with `include_str!`, and the `skill` command that writes it to stdout outside the envelope. |
| `src/exit.rs` | `Category`, the nine exit statuses, the single classifier from a stable dotted code to one of them, and the per-code sentence each `output.*` diagnostic explains itself with. |
| `src/commands/mod.rs` | The shared check, outcome and name views every command's report is built from. |
| `src/commands/inspect.rs` | The `inspect` report: observations, the declared document, and every check. |
| `src/commands/list.rs` | The `list` report: one row per archive entry, in central-directory order. |
| `src/commands/validate.rs` | The `validate-structure` report: the summary word, the checks, and the undecided rules. |
| `src/commands/extract.rs` | The `extract` report: the counts, one record per file written, and whether the run removed its marker. |
| `src/extract/mod.rs` | The four phases of a protected extraction — plan, preflight, write, commit — the marker name, and the failure policy that undoes this run's work. |
| `src/extract/preflight.rs` | What is decided before a byte is written: the destination, the marker, and every planned path against what is already there. Holds the link and reparse-point test. |
| `src/extract/writer.rs` | Exclusive creation of the marker, the directories and the files, each recorded as this run created it. |
| `src/extract/cleanup.rs` | The ledger of what this run created and the reverse-order undo, which never removes anything pre-existing. |
| `src/render/json.rs` | The one-object JSON envelope, in both its successful and its failed shape. |
| `src/render/human.rs` | Terminal-safe text: control, invisible and undecodable bytes are escaped and a long name is cut. |

## Format scope

KRX, as this project uses the word, means the ZIP-based document container
described by the primary sources registered in
[references.md](references.md) and reduced to numbered rules in
[profile.md](profile.md), whose rule-to-check map is
[conformance.md](conformance.md). It is the Hungarian realisation of the
SPOCS OCD container: a marker entry naming the format, a `Metalayer`
directory holding one descriptive metadata document, and payload
subdirectories holding one opaque attachment each.

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
| `max_text_bytes` | 1 MiB | character data and attribute values produced across the whole document |

`max_text_bytes` bounds one event as well as the document. Before a text or
CDATA event is unescaped and copied, and before an attribute value is
normalised, the event's raw byte length is checked against the headroom left
under the limit, so the copy materialised from a single event is bounded by
`max_text_bytes`. The raw event itself is read into the reader's buffer
before that check and stays bounded by `max_document_bytes`. Unescaping and
end-of-line normalisation never grow a value, so a raw length inside the
headroom guarantees the materialised one is too; the converse does not hold,
and an event whose raw form exceeds the headroom is refused even where its
materialised form would have fitted. A general entity reference needs no such
probe: only the five XML predefines and numeric character references resolve
at all, and each yields at most one character.

`max_document_bytes` matches `max_entry_decoded_bytes` because the document
reaches the parser through `ArchiveInventory::entry_bytes` and cannot be
larger than that ceiling. The grammar `KER_META_V0_9` describes needs six
levels of nesting and a few dozen elements, so the defaults are far above a
real document and far below what a general-purpose XML reader would accept.

### Extraction planning limits

`ExtractLimits::DEFAULT` carries these values. They bound the *output* an
extraction would produce and are independent of `Limits`, which bounds
reading the archive: a caller may accept an archive it will not extract.

| Limit | Default | Enforced against |
| --- | --- | --- |
| `max_files` | 256 | files the plan would create; directory markers do not count |
| `max_total_bytes` | 128 MiB | decoded bytes of all planned files, summed with checked arithmetic |
| `max_path_bytes` | 1024 | each destination path in UTF-8 bytes, separators included |
| `max_component_bytes` | 255 | each path component in UTF-8 bytes |
| `max_depth` | 16 | components in one destination path |

`max_files` and `max_total_bytes` match the archive-side `max_entries` and
`max_total_decoded_bytes`, because a plan can never describe more files, or
more bytes, than an accepted inventory holds; they are stated separately so a
caller can extract under a tighter bound than it reads under.
`max_component_bytes` is the smallest component length the common
filesystems agree on, and `max_path_bytes` and `max_depth` are sized far
above the layout rules A4, A5 and A10 imply.

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

## Extraction planning

`openkrx_core::extract::plan` turns an `ArchiveInventory` into an
`ExtractionPlan`: the list of files an extraction would create, decided
before anything is created. It is a pure function of the inventory and the
limits — no filesystem, clock, process or network access, allocation
proportional to the entry count, and the same plan for the same inventory
every time.

The split is deliberate. Every rule
[SECURITY.md](../SECURITY.md#required-threat-model-for-package-support)
requires of extraction that does **not** need a filesystem is decided here,
where it can be tested exhaustively without touching a disk. The filesystem
half — joining a plan onto a caller-selected destination, no-clobber
creation, the interrupted-write policy and the `extract` command — is in
`openkrx-cli` and described under
[Extraction output](#extraction-output). A `extract.*` code therefore does
reach the command line: it is what `extract` reports when the package could
not be planned at all.

A plan carries no `PathBuf`, no absolute path and no platform separator. A
`PlanItem` holds the entry index, the destination path as a `Vec<String>` of
components, the declared size and the decoded size; the plan holds the items
in central-directory order, the total decoded bytes, the deduplicated implicit
parent directories in sorted order, and the limits it was produced under. What
a path is, and where it is rooted, stays the caller's decision.

**A planning failure rejects the whole plan.** Nothing is skipped, renamed or
partially planned: a caller handed a quietly reduced plan would extract a
package that is not the package it was given. Every code is catalogued in
[codes.md](codes.md#extraction-planning-codes).

### Entry kinds

A ZIP archive declares what an entry *is* in two central-directory fields the
inventory now reports: `version made by`, whose high byte names the host
system, and `external file attributes`, whose meaning depends on that host.
`ArchiveEntry::kind()` maps them onto `EntryKind`:

| Condition | Kind |
| --- | --- |
| The name ends in `/` | `DirectoryMarker`, whatever the attributes say |
| Hosts 3 and 19 (Unix, OS X), `st_mode` in the high 16 attribute bits: `S_IFREG` / `S_IFDIR` / `S_IFLNK` | `RegularFile` / `DirectoryMarker` / `Symlink` |
| Hosts 3 and 19, any other mode, including a mode of zero | `Special` |
| Hosts 0, 10, 11, 14 (MS-DOS, NTFS, MVS, VFAT), FAT attribute bit `0x10` | `DirectoryMarker`, else `RegularFile` |
| Any other host system | `Unknown` |

Hosts 3 and 19 are the two APPNOTE names for a host that stores `st_mode` in
the high attribute bits, and they are read alike: a symlink written on macOS
must be refused as a link, not classified as `Unknown` and planned as an
ordinary file holding its target text. No other host is assumed to carry a
mode, and none is added on a guess.

A `Symlink` and a `Special` entry reject the plan, with
`extract.unsupported.link` and `extract.unsupported.special_file`: a link is
never created, and its target text is never written as an ordinary file
either. A `DirectoryMarker` produces no item. An `Unknown` entry is planned as
an ordinary file — a name ending in `/` has already been classified as a
directory, so nothing about the name is being assumed — and a `Special`
result from a Unix entry whose mode states no file type is a refusal rather
than a guess.

### Path rules

The entry name must be valid UTF-8 (`extract.unsupported.non_utf8_name`);
rule A21 leaves the intended encoding unresolved, so no code page is guessed.
The name is split on `/`, the only separator rule A8 permits, and each
component is checked against the union of the three target platforms' rules,
not the rules of the platform the planner happens to run on. A component may
not be empty, `.` or `..`, may not hold a NUL, another C0 control or a C1
control character, may not end with `.` or a space, may not be a Windows
reserved device name (`CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9`,
`LPT1`–`LPT9`, with or without an extension, compared ASCII
case-insensitively), may not start with a space, may not hold `:`, and may not
hold any of the six characters no Windows filesystem accepts in a name — `*`,
`?`, `<`, `>`, `|` and `"`, which share one
`extract.unsafe_path.reserved_character` code. A backslash is not among them:
`archive::names` refuses such a name outright, before a plan is attempted.
Each violation has its own `extract.unsafe_path.*` code, and the diagnostic
carries the entry index only, never the component.

Several of those classes — `..`, a C0 control, an absolute first component —
are already impossible in an accepted inventory, because `archive::names`
refuses such a name outright. They are checked again because the planner's
output must be safe on its own terms rather than because an earlier layer is
assumed to have run.

### Collision policy

Two outputs that a filesystem could not keep apart are **refused, never
resolved**, as
[SECURITY.md](../SECURITY.md#required-threat-model-for-package-support)
requires. Renaming one would hand the caller a file whose name is not the
name the package declares; skipping one would report success over a partial
result.

- Two destination paths equal after NFC normalisation and case folding are
  `extract.ambiguous.collision`. That is what a normalising or
  case-insensitive filesystem — APFS, NTFS, a case-insensitive ext4
  directory — would see. Case folding uses `char::to_lowercase`, the Unicode
  *simple* lowercase mapping, which approximates full case folding; the
  residual risk is recorded in
  [SECURITY.md](../SECURITY.md#threat-model-mapping-extraction-planning-layer).
  A byte-identical or plainly case-folded duplicate never reaches the
  planner: the inventory already refuses it as
  `archive.ambiguous.duplicate_name` or `.case_folded_duplicate_name`.
  Normalising to NFC first is what closes the remaining gap.
- A path that is a directory prefix of another output is
  `extract.ambiguous.file_directory_conflict`: `a` and `a/b` cannot both
  exist. The reported entry is the later of the two.

Directories in a plan are implicit: they are the parents of planned files,
deduplicated and sorted component-wise so a parent precedes its children. An
empty directory is deliberately **not** materialised, so a directory marker
with nothing beneath it produces nothing at all.

## Extraction output

`extract` is the only command that writes, and
`crates/openkrx-cli/src/extract/` is the only code in openKRX that touches a
filesystem for output. It adds no rule about names, entry kinds, collisions
or ceilings: those belong to the planner above, and are enforced before this
layer runs. What it owns is the destination.

```text
openkrx extract <FILE|-> --into <DIR> [--json]
```

`<DIR>` must already exist, must be a directory, and must not be a symbolic
link or a Windows reparse point; all three are checked with
`symlink_metadata`, which does not follow a link. It need **not** be empty.
`extract` never creates its destination: creating one would mean choosing
where, with which parents and with which permissions, and a typo in the
argument would silently produce a tree instead of a refusal.

A run has four phases, and each finishes before the next begins.

**1. Plan.** `archive::inventory`, then `extract::plan` under
`ExtractLimits::DEFAULT`. A planning refusal rejects the whole package with
its own `extract.*` code and its own category — an unsupported entry is still
a 7, a ceiling still an 8 — and nothing has touched the destination.

**2. Preflight.** The destination, then the marker, then every planned path.
For each planned file, every ancestor inside the destination that exists must
be a real directory (`output.symlink_in_path`, `output.not_a_directory`) and
the file path itself must not exist **in any form** — file, directory, link,
or a link whose target is missing (`output.exists`). A package may declare an
entry named `.openkrx-extract.partial`, as a file or as a root directory
holding entries beneath it; the planner has no opinion about it, so preflight
refuses either under the same code rather than letting it collide
with this run's own marker as an I/O failure part-way through. A preflight refusal
means nothing was written, and the JSON report says so with `removed: 0`.
A *nested* `x/.openkrx-extract.partial` is extracted normally, with the side
effect that a later `extract --into <dest>/x` is refused with
`output.partial_marker_present`: the marker name is fixed, so any package may
place one where a future run would read it as its own.

**3. Write.** A `.openkrx-extract.partial` marker is created in the
destination with `create_new`, before anything else. Then each planned
directory in plan order — which lists every implicit parent, sorted so a
parent precedes its child — with `create_dir`, never `create_dir_all`, and
each is re-read with `symlink_metadata` after creation. A directory that
already existed is neither counted nor recorded: this run did not create it,
so this run must never remove it. Then each planned file with
`OpenOptions::new().write(true).create_new(true)`, which is `O_EXCL` on Unix
and `CREATE_NEW` on Windows: it refuses rather than truncating, and it does
not follow a symbolic link at the leaf. Bytes come from
`ArchiveInventory::entry_bytes`, bounded by `max_entry_decoded_bytes` and
CRC-checked, and are written in full. **No permission bit and no timestamp is
copied from the archive**: a package is untrusted input, and the process
umask, or the destination's inherited Windows ACL, decides instead.

**4. Commit.** Removing the marker is the last step. A destination that still
holds `.openkrx-extract.partial` was not finished, and a consumer must treat
it as incomplete.

### Cleanup after a failed write

Every path this run creates — files, directories and the marker — is recorded
in creation order. Any failure after the first write walks that record
backwards and removes exactly those paths: `remove_file` for a file,
`remove_dir` for a directory, never `remove_dir_all`, so a directory that has
since acquired something this run did not create survives instead of being
deleted with its contents. **Nothing pre-existing can be removed**, because
nothing pre-existing was ever recorded.

The command then exits with the failure's own category, and the report says
what the pass did: `removed` is how many of this run's paths are gone,
`left_in_place` how many could not be removed and are still there. Human mode
prints the same as a second line on standard error. Neither is a path.

**Residual limitation.** A crash — a signal, a power loss, a killed process —
cannot run the undo pass, so it leaves partial files and the marker behind.
That is what the marker is for: the next run into that destination is refused
with `output.partial_marker_present` rather than mixing two runs' output, and
clearing it is a deliberate human act. Rename-based staging — writing into a
temporary tree and renaming it into place — would make the whole extraction
atomic, and is deliberately deferred: it needs a second confined destination
on the same filesystem, doubles the peak space, and moves the no-clobber
question to the rename rather than removing it.

### Race assumptions, and what they do not cover

The destination is trusted not to be modified by another principal while the
command runs. Exclusive creation and the post-creation `symlink_metadata`
checks defend against what is **already** at the destination — an existing
file, a symbolic link, a Windows reparse point — and not against an attacker
holding concurrent write access to it, who can win the window between a check
and the operation that follows it. Closing that window needs `openat2` with
`RESOLVE_BENEATH` on Linux, or the equivalent per-platform primitive, and is
deliberately deferred; it is recorded as a residual risk in
[SECURITY.md](../SECURITY.md#residual-risks-of-the-output-layer).

Per platform: Windows reparse points are detected through
`FILE_ATTRIBUTE_REPARSE_POINT` as well as `is_symlink`, because a junction
does not always report as a symbolic link; reserved device names, trailing
dots and trailing spaces are already refused by the planner, so no output
name can collapse onto another one there. Case-insensitive filesystems and
APFS normalisation are covered by the planner's NFC and case-folded collision
rejection, and by the no-clobber rule as the second line: two paths a
filesystem equates cannot both be created with `create_new`.

## Command contract and JSON envelope

The supported surface is `openkrx --help`, `openkrx --version`,
`openkrx capabilities [--json]`, the three reader commands, `extract`, and
`skill`:

```text
openkrx inspect            <FILE|-> [--json]
openkrx list               <FILE|-> [--json]
openkrx validate-structure <FILE|-> [--json]
openkrx extract            <FILE|-> --into <DIR> [--json]
openkrx skill
```

| Command | Input | Output | Envelope | Exit statuses |
| --- | --- | --- | --- | --- |
| `capabilities` | none | stdout | yes, with `--json` | 0, 2 |
| `inspect` | one package | stdout | yes, with `--json` | 0, 2, 5, 6, 7, 8 |
| `list` | one package | stdout | yes, with `--json` | 0, 2, 5, 6, 7, 8 |
| `validate-structure` | one package | stdout | yes, with `--json` | 0, 2, 3, 4, 5, 6, 7, 8 |
| `extract` | one package | stdout and `--into <DIR>` | yes, with `--json` | 0, 2, 5, 6, 7, 8, 9 |
| `skill` | none | stdout | **no: it bypasses the envelope** | 0, 2 |

**`skill` is outside the envelope.** It writes the agent skill document
embedded in the binary — the same bytes as
`crates/openkrx-cli/skills/openkrx/SKILL.md`, by `include_str!` — to stdout
byte for byte, and nothing else: no `schema_version`, no `ok`, no `verified`,
and nothing on stderr. It reads no package, so it takes no `FILE` and no
`--json`, and passing either is a usage error that exits 2; a successful run
exits 0. It is not a package operation and does not appear in
`capabilities().operations`. Wrapping a Markdown document in a JSON string
would only make the one thing a caller wants harder to reach, and there is no
package for a `command`, `data` or `verified` field to be about.

The three reader commands write nothing anywhere. `extract` writes exactly
the files the package declares, into the directory `--into` names, under the
rules in [Extraction output](#extraction-output).

Each reader command takes exactly one input: a path, opened exactly as
written with no normalisation, globbing or extension inference on any
platform, or `-` for standard input, read as binary on every platform. There
are no limit-override flags, no colour or TTY detection, no configuration
file and no shell completions; the same arguments produce the same bytes on
every supported system. This is a reader, not a conformance checker: what
its output may state follows the rules in [profile.md](profile.md) and the
rule-to-check map in [conformance.md](conformance.md).

**Bounded input.** At most `Limits::DEFAULT.max_archive_bytes + 1` bytes are
ever buffered. An input that reaches that cap is refused with
`input.over_limit.archive_bytes` before any parsing begins, so naming a very
large file cannot be turned into memory pressure. The core crate performs no
I/O: the executable reads the bytes and calls `archive::inventory`, then
`profile::check`.

**One object, one stream.** In JSON mode stdout carries exactly one JSON
object and nothing else, and stderr is empty on success. In human mode stdout
carries the report and stderr carries at most one diagnostic line. `inspect`
and `list` exit 0 whenever they produce their report; only
`validate-structure` encodes the structural reading in its status.

**Privacy.** A diagnostic — on stderr, and the `error` object — carries a
stable code, its category, an entry index and numbers. It never carries the
input path, an entry name or a metadata value. Declared metadata values are
printed by `inspect` only, on stdout, because that is what it was asked for.
Nothing is logged, cached or written anywhere.

**Terminal safety.** Human mode escapes C0 and C1 controls, `DEL`, the
byte-order mark and the invisible and bidirectional formatting characters as
`\u{..}`, and a byte belonging to no valid UTF-8 sequence as `\x{..}`; rule
A21 leaves entry-name encoding unresolved, so no encoding is guessed. A name
longer than 200 characters is cut and the number of characters dropped is
stated as `…[+N]`. JSON output is never truncated: a consumer needs the whole
name, and a name that is not valid UTF-8 is reported as `name_hex` rather
than as an invented string. Output is bounded without a further rule, because
the inventory refuses more than 256 entries and a name longer than 255 bytes.

### The successful envelope

```json
{
  "schema_version": 1,
  "ok": true,
  "command": "capabilities",
  "data": {
    "project": "openKRX",
    "stage": "reader",
    "operations": ["inspect", "list", "validate-structure", "extract"]
  },
  "verified": false
}
```

Without `--json`, `capabilities` prints the project and stage, the operation
list, and the boundary sentence. `operations` lists implemented package
operations. `verified: false` expresses the cryptographic boundary and is
never `true` in this design, in any response.

**`ok` is about the command, not about the package.** It says that stdout
carries a report rather than a diagnostic. It stays `true` when a structural
check failed, because the failure is in the report; `validate-structure` on a
package with a failing check writes `"ok": true` and
`"summary": "inconsistent"` and exits 3. A consumer deciding what to do about
a package therefore reads `summary`, or the exit status, and never `ok`. The
field name invites the other reading, so it is stated here, in
`openkrx --help` and in `openkrx validate-structure --help`; renaming it
would remove a field from the envelope, which would raise `schema_version`.

### The failed envelope

Every failure exits with the status its category names, in both modes. In
JSON mode the single object on stdout is:

```json
{
  "schema_version": 1,
  "ok": false,
  "command": "list",
  "error": {
    "code": "archive.over_limit.entries",
    "category": "limit",
    "limit": 256,
    "observed": 300
  },
  "verified": false
}
```

`entry_index`, `limit` and `observed` are present only when the failure
carries them. Both modes also write one short line on stderr:
`openkrx: <code>`, followed by the same numbers.

### `list`

`entries[]`, in central-directory order, which is stable across runs and
platforms. `name` is present when the name bytes are valid UTF-8 and
`name_hex` only when they are not; `name_utf8_flag` reports general-purpose
bit 11 independently of whether the bytes decode. `declared_size` is what the
central directory says and `decoded_size` is what the decoder produced; they
are reported side by side and never collapsed into one figure.

```json
{
  "schema_version": 1,
  "ok": true,
  "command": "list",
  "data": {
    "entries": [
      {
        "index": 0,
        "name": "mimetype",
        "name_utf8_flag": false,
        "method": "stored",
        "compressed_size": 19,
        "declared_size": 19,
        "decoded_size": 19,
        "crc32": "10b6eee7"
      }
    ]
  },
  "verified": false
}
```

### `inspect`

`observations` are archive-shaped facts, `metadata` is the declared document
when one was located and parsed and `null` otherwise, and `checks[]` is the
full inventory in `CheckId::ORDER`. Nothing is interpreted: `created_at`
keeps its lexical form because the crate has no clock, and
`declared_size_text` is authoritative because rule M13 leaves the unit and
rounding of `MERET` open. `declared_size_value` is present only when `MERET`
reads as a finite number.

```json
{
  "schema_version": 1,
  "ok": true,
  "command": "inspect",
  "data": {
    "observations": {
      "entry_count": 3,
      "root_prefix": "KRX/OCD/",
      "metadata_entry_name": "KRX/OCD/Metalayer/KULDEMENY_META.xml",
      "metadata_entry_index": 1,
      "marker": {"outcome": "pass"}
    },
    "metadata": {
      "version": "v0.9",
      "source_system": "KER",
      "consignment_type": "KULDEMENY",
      "consignment_id": "SYN-0001",
      "created_at": "2026-01-02T03:04:05.000+01:00",
      "test": false,
      "test_present": true,
      "declared_attachment_count": 1,
      "attachments": [
        {
          "number": 1,
          "declared_path": "KRX/OCD/Payload/ID-1/synthetic.pdf",
          "resolution": "resolved",
          "entry_index": 2,
          "declared_size_text": "12.5",
          "declared_size_value": 12.5,
          "observed_size": 19
        }
      ]
    },
    "checks": [
      {"check": "metadata_location", "outcome": "pass"},
      {"check": "declared_size", "outcome": "unresolved", "rule": "M13"}
    ]
  },
  "verified": false
}
```

`reference_id`, `barcode` and `error_code` appear only when the document
declares them, and `root_prefix_hex` and `metadata_entry_name_hex` only when
those names are not valid UTF-8. A check carries `code` only when it failed
and `rule` only when it is undecided; the four outcomes are `pass`, `fail`,
`unresolved` and `not_applicable`, and they stay distinct in every rendering.

### `validate-structure`

`summary` is `consistent`, `inconsistent` or `unresolved` — the summary of
[the check inventory](#structural-check-inventory), and **not** a conformance
verdict. `unresolved_rules[]` names each undecided rule once, in the order
the checks first cite it.

```json
{
  "schema_version": 1,
  "ok": true,
  "command": "validate-structure",
  "data": {
    "summary": "unresolved",
    "checks": [
      {"check": "metadata_location", "outcome": "pass"},
      {"check": "attachment_references", "outcome": "fail",
       "code": "metadata.reference.missing_entry"},
      {"check": "declared_size", "outcome": "unresolved", "rule": "M13"}
    ],
    "unresolved_rules": ["M13"]
  },
  "verified": false
}
```

### `extract`

`items[]` records one file per entry written, in plan order, which is
central-directory order. `path` is relative to the destination, with its
components joined by `/` on every platform; the destination itself is never
reported, because the caller named it and a report that repeats it cannot be
logged unedited. `directories_created` counts only directories this run
created, not ones that were already there, and `marker_removed` says the run
reached its last step.

```json
{
  "schema_version": 1,
  "ok": true,
  "command": "extract",
  "data": {
    "files_written": 3,
    "directories_created": 5,
    "bytes_written": 1041,
    "items": [
      {"entry_index": 0, "path": "mimetype", "bytes": 19},
      {"entry_index": 2, "path": "KRX/OCD/Payload/ID-1/synthetic.pdf", "bytes": 19}
    ],
    "marker_removed": true
  },
  "verified": false
}
```

`items[].path` is the only place a name from the package reaches the output
of a command other than `inspect`, and it appears **only on success**. A
failed `extract` carries the ordinary failed envelope plus one extra object:

```json
{
  "schema_version": 1,
  "ok": false,
  "command": "extract",
  "error": {"code": "output.exists", "category": "output", "entry_index": 2},
  "cleanup": {"removed": 0, "left_in_place": 0},
  "verified": false
}
```

`cleanup` is present for `extract` alone. It counts paths this run had
created and then removed, and paths it created and could not remove; both
zero means nothing had been written when the run was refused. Human mode
prints the same fact as a second line on standard error. Neither carries a
path, an entry name or an operating-system message.

There is deliberately no field named `valid`, `conforming` or `is_krx` in any
response, and no command prints such a word as a claim.

**The `schema_version` compatibility rule.** `schema_version` is `1`. A
consumer must ignore object fields it does not recognise, because adding a
field is not a breaking change and will not raise the version. A field being
removed, renamed, or given a different type or meaning is a breaking change,
and requires an explicit decision to raise `schema_version` in the same
change that makes it. A consumer that reads an unknown `schema_version`
must stop rather than guess.

## Exit statuses

Nine categories, exhaustive and stable; a consumer may branch on the number.
`crates/openkrx-cli/src/exit.rs` holds the only mapping, and one unit test per
category holds each row.

| Status | Category | Meaning |
| --- | --- | --- |
| 0 | `success` | The command produced its report. For `validate-structure`, the summary is `consistent`. |
| 2 | `usage` | The arguments were rejected: an unknown or missing command, a missing file argument, or an invalid flag. |
| 3 | `structure_inconsistent` | `validate-structure` only: at least one check failed. |
| 4 | `structure_unresolved` | `validate-structure` only: no check failed, and at least one rule could not be decided. |
| 5 | `input` | The input could not be opened or read, or it reached the input cap. |
| 6 | `package` | The package is malformed, truncated or ambiguous, or an entry name is unsafe. |
| 7 | `unsupported` | The package uses a feature this reader does not implement: ZIP64, encryption, a multi-disk archive, another compression method, or an XML feature the parser refuses. |
| 8 | `limit` | A documented parsing or extraction limit was exceeded. |
| 9 | `output` | `extract` only: the destination could not be used, or a write failed. Nothing incomplete was left behind. |

`inspect` and `list` never exit 3 or 4: a failing or undecided check is part
of their report, not their status. A structural failure is therefore visible
in three ways that never disagree — the check's outcome, the summary word,
and the exit status — but only `validate-structure` puts it in the status. A
reader who took the table for a blanket rule would run `inspect`, check `$?`,
and take a broken package for a clean one, so the table is printed under
`openkrx --help` with the two `validate-structure`-only rows marked as such,
and `openkrx inspect --help` and `openkrx list --help` each repeat their own
rule. Statuses 3 and 4 exist to be scripted against; `inspect` and `list`
exist to be read.

In human mode the diagnostic line names the status and says what the category
means, in one line that still carries no part of the input, for example:
`openkrx: archive.unsupported.zip64 at entry 0 — the package uses a ZIP or
XML feature openkrx does not implement; it is not damaged, and another reader
may open it (exit 7)`.

A code classifies to exactly one category, by its head and its category
segment: `input.*` to 5; `*.over_limit.*` to 8, except
`input.over_limit.archive_bytes`, which is an input problem because nothing
was parsed at all; `*.unsupported.*` to 7; `*.truncated.*`, `*.malformed.*`,
`*.ambiguous.*`, `archive.unsafe_name.*`, `extract.unsafe_path.*` and
`archive.no_such_entry` to 6; and every `output.*` code to 9. The remaining
`metadata.*` codes are structural-check failures, which reach a status only
through `validate-structure`'s summary, as 3.

Statuses 3, 4 and 9 each belong to one command. 9 is `extract`'s alone, and
it says something the other statuses do not: the destination was left as it
was found, either because the run was refused before writing or because the
undo pass removed everything it had created. In human mode the second line on
standard error states which of the two happened.

The core error enums are `#[non_exhaustive]`, so no downstream `match` on
their variants can be exhaustive and a new code cannot be made to fail
compilation in the command-line crate. Classification therefore keys on the
code's own category segment, which is part of the published contract, and a
segment this build does not know classifies to nothing. A test reads every
code out of [codes.md](codes.md) — the catalogue `scripts/check-codes.py`
forces to stay complete — and fails when one of them does not classify, so an
added code cannot reach a release unclassified.

## Stable codes

Every failure openKRX produces carries a stable dotted code. The archive
layer produces `archive.*`, the metadata layer and the profile layer both
produce `metadata.*`, and the two `metadata.*` sets are disjoint. The
command-line crate adds `input.*` for the one thing the core crate cannot
fail at, because it performs no I/O: reading the input. A consumer can
bucket every diagnostic by its dotted code alone.

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
