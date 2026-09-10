# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Nothing has been released yet: both crates set `publish = false` and there
is no tag. While the project is pre-1.0, the JSON envelope is versioned
separately by its `schema_version` field, which is `1`. Adding a field to
that envelope does not raise it; removing, renaming or redefining one does,
and such a change is recorded here explicitly.

## [Unreleased]

### Added

- **`openkrx create`: writing one package from a manifest file (KRX-06,
  command half).** `openkrx create --manifest <FILE> --out <FILE.krx>
  [--json]` reads one JSON manifest, reads the local files it names as
  attachments, and writes the bytes `openkrx_core::create::package` produces
  to a file that must not already exist. `--stdout` writes the package to
  standard output instead and puts the report, JSON object included, on
  standard error. openKRX reads no clock, so the manifest's `timestamp` is
  required and the same manifest and files always produce byte-identical
  output.

  The manifest is one object carrying `schema_version` 1, a `metadata`
  object mirroring the document's own fields, an optional `attachments`
  array of local files, and `timestamp`. It is validated strictly in both
  directions: a required field that is absent is refused, and **a key the
  schema does not define is refused rather than ignored**, because a
  misspelled `attachments` would otherwise write a package with no
  attachment and report success. An attachment path is resolved against the
  manifest's own directory; the name inside the package is its last
  component, or the `file_name` the manifest gives, and goes through the
  writer's name rules either way. The schema is documented in
  [docs/architecture.md](docs/architecture.md#creating-a-package).

  The output rule is `extract`'s: `--out` must not exist in any form, its
  parent must already be a real directory that openKRX never creates, the
  file is created with `create_new`, and a failure after that removes it
  again and reports the removal in `cleanup`. After writing, openKRX reads
  the file back through `archive::inventory`, `metadata::parse` and
  `profile::check`; a failing check is a defect in openKRX, reported as
  `create.internal.self_check_failed` with the file removed.

  **A package `create` wrote passes `validate-structure` with exit 4, and
  never 3.** Nothing fails; the marker's place cites unresolved rule A19 and
  any declared attachment size cites M13. That is the documented definition
  of success. The layout is the canonical documented one and is
  **unverified against every real producer**: a created package is
  structurally consistent with the documented layout, never conforming, not
  signed, and not something any receiving service has agreed to accept.

- **A deterministic package writer, in the library only (KRX-06, core half).**
  `openkrx_core::create::package` turns a `PackageSpec` — typed metadata,
  attachment bytes, a caller-supplied `FixedTimestamp` and a `Layout` — into
  the bytes of one package. It is a pure function: no filesystem, clock,
  process or network access, no randomness, and equal inputs produce
  byte-identical output. The entries are `KRX/OCD/mimetype` first and stored,
  `KRX/OCD/Metalayer/KULDEMENY_META.xml`, then
  `KRX/OCD/Payload/ID-<n>/<file>` in request order, deflated at one fixed
  level, with bit 11 set on every name, host 3 and mode `0o100644`, and no
  directory entry, extra field, data descriptor, comment or ZIP64 record.
  The `MELLEKLET` references and `MELLEKLETEK_SZAMA` are derived from the
  attachments actually written — `MERET` in kilobytes rounded up, the unit
  M6 documents — and a caller-supplied value that disagrees is refused. The
  reader's `Limits` — entry count, name length, per-entry and total decoded
  bytes, compression ratio and image length — and both the archive and
  extraction name rules are enforced on the output, so whatever `package`
  returns is a package the reader accepts; `create::verify_round_trip` reads
  a written one back through `archive::inventory`, `metadata::parse` and
  `profile::check`. No command exposes any of this: nothing writes a file,
  and `capabilities().operations` is unchanged.

  **The writer emits the canonical documented layout. Interoperability with
  real producers is unverified because rules A19–A22 and M11–M15 remain
  unresolved; the reader's structural checks are the only gate, and a
  written package is "structurally consistent with the documented layout",
  never "conforming".** Reading one back still reports `Unresolved(A19)` for
  the marker's location and `Unresolved(M13)` for the declared size.
- **Twenty-three stable `create.*` codes**, catalogued in
  [docs/codes.md](docs/codes.md#creation-codes) and extracted by
  `scripts/check-codes.py`, whose head list gains `create`:
  `create.invalid.*` for a request that contradicts itself,
  `create.over_limit.*` for a ceiling the output would exceed — including the
  compression ratio the reader refuses a decompression bomb by — and
  `create.unsafe_name.*` for a name the reader or the extraction planner
  would refuse. `Category::of_code` classifies the first and third as a
  package problem (exit 6) and the second as a limit (exit 8).

- **An opt-in private-corpus harness that reports only aggregate counts.**
  `scripts/private-corpus.py --bin <path> --dir <directory>` runs the built
  executable's `inspect`, `list` and `validate-structure` over a
  maintainer-local directory of real `.krx` packages and prints counts alone:
  packages read, exit status and `error.code` and `error.category` per
  command, every structural check as check, outcome and code-or-rule, the
  root-prefix, metadata file-name casing, format-marker position and payload
  subdirectory spelling classes, an entry-count histogram, and the runs that
  timed out or produced no envelope. It never prints a file name, entry name,
  path, metadata value, hash, timestamp or identifier — every label is a
  fixed class, check, outcome, rule or stable code — and it refuses a
  directory inside the repository tree unless `--allow-in-repo` is given.
  Nothing invokes it: not `scripts/check.sh`, not CI, not `cargo test`.
  `--self-test` copies the five committed golden fixtures under loud file
  names, asserts the buckets they must produce, and asserts that no canary —
  file name, temporary directory, synthetic identifier, attachment name or
  metadata entry name — reaches the output.

- **A held release workflow: multi-platform builds, checksums and build
  provenance.** `.github/workflows/release.yml` is hand-maintained rather than
  generated, so the file a reviewer reads is what runs. On a `v*` tag it builds
  `openkrx-cli` for six targets — `x86_64` and `aarch64` Linux (gnu, plus musl
  on `x86_64`), both macOS architectures and `x86_64` Windows — each on a runner
  of its own architecture, so no cross toolchain, emulator or container is in
  the trust path and every archive is executed before it ships. Each archive
  carries the binary, `LICENSE`, `README.md`, `CHANGELOG.md` and a `SKILL.md`
  written out by the packaged binary itself, beside one `SHA256SUMS` over all
  six. A per-target smoke job verifies the checksum, unpacks the archive and
  runs the packaged binary: `--version` against the workspace version,
  `capabilities --json` against `schema_version` 1 and the exact operation list,
  `skill` against its front matter, and `validate-structure` over
  `tests/fixtures/golden/consistent.krx` asserting **exit status 4** and the
  `unresolved` summary, so a binary that started emitting a conformance verdict
  never reaches a release. `actions/attest-build-provenance` then attests the
  archives — SLSA build provenance signed by GitHub's Actions identity, **not**
  a signature by a maintainer and not a code-signing certificate. **Nothing is
  published.** The workflow creates a *draft* release and stops; a human
  publishes it. It creates no tag, touches no registry, and neither removes nor
  weakens `publish = false`, and `workflow_dispatch` with `dry_run` (default
  `true`) rehearses the entire build and smoke path from any branch, uploading
  workflow artifacts and creating nothing — `dry_run: false` is refused
  outright, because a release is cut by pushing a tag. A pull request changing
  `release.yml` or `release-notes.py` rehearses the same way, behind a path
  filter narrow enough to keep six native builds off every other pull request,
  and that trigger is also the only way to exercise a release workflow that has
  not reached the default branch yet. The attestation and draft-release jobs
  are guarded twice: by the computed `dry_run` value, and by a direct
  `refs/tags/v` test on the trigger that no later change to that logic can
  talk its way past. `plan` compares the tag against `cargo metadata`'s
  workspace version and fails the run if they disagree; permissions are per-job,
  with `contents: write` only in the draft-release job and
  `id-token`/`attestations: write` only in the attestation job; every `uses:` is
  pinned to a full commit SHA with its version in a comment, reusing `ci.yml`'s
  SHAs where the action is shared. The new stdlib script
  `scripts/release-notes.py` slices the release notes out of this file — a
  version section on a tag, `[Unreleased]` on a dry run — and exits 1 when the
  section is missing or empty, with a `--check` mode for CI.
  [docs/releasing.md](docs/releasing.md) is now the runbook, with the evidence
  gate still standing first: the automation is ready, the release is not.

- **An embedded agent skill, and the `skill` command.**
  `crates/openkrx-cli/skills/openkrx/SKILL.md` states the rules an AI agent
  follows when driving openkrx: bounded local reading, no conformance,
  validity or authenticity claim, attachments treated as opaque bytes,
  nothing uploaded, and `verified` always `false`. It states the boundary
  once — structural checks are not signature verification, `consistent` is
  not conformance, and nine rules are unresolved — documents the JSON
  envelope and all nine exit statuses, and gives the
  `capabilities` → `inspect` → `list` → `validate-structure` → `extract`
  workflow with how to read `checks[]`, `unresolved_rules`, `items` and
  `cleanup`. `openkrx skill` writes that document to stdout byte for byte,
  by `include_str!`, so it ships inside the binary and cannot drift from it.
  The command bypasses the JSON envelope, takes no `FILE` and no `--json`,
  exits 0, and exits 2 on either; it is not a package operation and
  `capabilities().operations` is unchanged.
- **Mutation testing, with a per-crate caught-mutant floor.**
  `cargo mutants` rewrites one expression at a time and reruns the suite
  against each mutated copy, which measures whether a test would notice a
  behaviour change rather than only that a line ran.
  [.cargo/mutants.toml](.cargo/mutants.toml) bounds the run and excludes the
  test-only synthetic writer, the examples directory and the separate fuzz
  package; `scripts/mutants_gate.py` reads the report and enforces the
  per-crate floors in `scripts/mutants-floors.txt`, counting a timeout or an
  unviable mutant on neither side; and `.github/workflows/mutants.yml` runs
  both weekly and on demand, uploading the report and, by design, gating no
  pull request. The first run's survivors were triaged in the same change:
  the genuine gaps became boundary and rendering tests — central-directory
  accessors, both halves of the M11 completeness rule, resolution through a
  root prefix other than the observed one, the compression ratio at exactly
  its limit, the ZIP64 entry sentinel in the end record, a directory marker
  carrying a compressed payload, the optional sibling blocks one at a time,
  the human renderer line by line, and the sentence each `output.*` code is
  given — and the rest are listed with their reasons in
  [docs/testing.md](docs/testing.md#mutation-testing).

- An unpublished Rust workspace, `openkrx-core` and `openkrx-cli`, on
  edition 2024 with a minimum supported Rust version of 1.88. The
  executable offers `--help`, `--version` and `capabilities [--json]`; the
  JSON response is one object carrying `schema_version`, `ok`, `command`,
  `data` and `verified: false`, and its `operations` list is empty because
  no package operation exists. It carries the development checks, the CI
  workflow, and the contribution, security and fixture policies.
- **The first supported KRX profile, as evidence (KRX-01).**
  [docs/profile.md](docs/profile.md) records twenty-two archive rules and
  fifteen metadata rules extracted from four primary Magyar Posta documents
  and the `KER_META_V0_9` schema embedded in one of them, each with a
  section-level citation and an evidence class: normative, service,
  example, or unresolved. Nine rules are unresolved because the sources
  contradict each other or are silent, and the document states what that
  blocks. [docs/references.md](docs/references.md) records each source's
  retrieval status and copyright terms; no schema, sample or document text
  is committed, because none of the sources grants redistribution.
- **A bounded, profile-agnostic archive inventory (KRX-02).**
  `openkrx_core::archive::inventory` reads a caller-supplied ZIP byte slice
  and reports what the archive contains. It performs no filesystem, clock,
  process or network access. Eight documented limits — archive bytes, entry
  count, name length, per-entry and total decoded bytes, compression ratio,
  extra-field bytes and comment bytes — are enforced against bytes the
  decoder actually produced, not against declared sizes, in 64 KiB
  increments, so no limit can be overshot by more than one buffer. Every
  byte of the image must be claimed by exactly one declared structure, the
  central directory is authoritative over local headers and data
  descriptors, and ambiguity is refused rather than resolved: a second
  terminating end record, a duplicate entry name and a case-folded name
  collision are each an error. Stored and deflate are the supported
  methods; ZIP64, encryption, patched data, multi-disk archives and every
  other method are refused with their own codes. Forty-three stable dotted
  codes in six categories keep truncation, self-contradiction, ambiguity,
  unsupported features, resource exhaustion and unsafe names
  distinguishable, and a diagnostic prints the code, an entry index and
  numeric limit values only, never an entry name or entry content. No CLI
  command exposes any of this, so `capabilities().operations` stays empty.
- **Bounded metadata parsing and a structural check inventory (KRX-03).**
  `openkrx_core::metadata::parse` reads one `KER_META_V0_9`-shaped XML
  document against the grammar rules M1 to M8 describe. No XSD is vendored;
  the grammar is re-expressed in code, citing the rule each function
  encodes. A `<!DOCTYPE ...>` declaration, a general entity reference other
  than the five XML predefines, a processing instruction other than the XML
  declaration, and a declared encoding other than UTF-8 are each refused
  with their own stable code rather than processed. Five limits — document
  bytes, depth, element count, attributes per element and total character
  data — are counted inside the event loop against values actually
  produced. `openkrx_core::profile::check` then runs eleven named checks in
  a fixed order over an inventory and its metadata document, reporting each
  as `Pass`, `Fail(code)`, `Unresolved(rule)` or `NotApplicable`, alongside
  the observations it made and one resolution per declared attachment.
  Thirty further stable codes were added. **No conformance verdict exists
  and none is emitted:** each unresolved profile rule maps to a distinct
  `Unresolved` outcome citing that rule, never to a pass and never to a
  failure, and the summary value `Consistent` means only that no check
  failed and none was left open. There is deliberately no `valid`,
  `conforming` or `is_krx` field anywhere in the API. No CLI command
  exposes any of this either.
- **The three reader commands (KRX-04).** `openkrx inspect`, `openkrx list`
  and `openkrx validate-structure` each read one package — a named file, or
  `-` for standard input, read as binary on every platform — and report what
  is in it. `list` reports every archive entry in central-directory order
  with its declared and its decoded size side by side; `inspect` adds the
  declared metadata document and every structural check; `validate-structure`
  reports the check inventory outcome by outcome. `--json` writes exactly one
  object on stdout, `schema_version` 1, and leaves stderr empty on success.
  Input is bounded before parsing: at most
  `Limits::DEFAULT.max_archive_bytes + 1` bytes are ever buffered and
  anything larger is refused with `input.over_limit.archive_bytes`. Eight
  exit statuses — 0 success, 2 usage, 3 a check failed, 4 a rule could not be
  decided, 5 input, 6 malformed package, 7 unsupported feature, 8 resource
  limit — are published in
  [docs/architecture.md](docs/architecture.md#exit-statuses) and classified in
  one place. `capabilities` now reports stage `reader` and names exactly
  these three operations. **No verdict is emitted:** there is no `valid`,
  `conforming` or `is_krx` field, `verified` stays `false` everywhere, and a
  summary of `consistent` means only that no check failed and none was left
  undecided. A diagnostic carries a stable code, a category, an entry index
  and numbers, and never the input path, an entry name or a metadata value;
  human output escapes control, invisible and undecodable bytes and cuts a
  name longer than 200 characters, and each diagnostic line names its status
  and says what its category means. `openkrx --help` publishes the exit-status
  table, marking the two statuses that belong to `validate-structure` alone,
  and states that the envelope's `ok` reports only that a report was produced:
  it stays `true` when a structural check failed, so a consumer reads
  `summary`, or the exit status, to act on the checks. Two stable codes,
  `input.unreadable` and `input.over_limit.archive_bytes`, were added, and
  `scripts/check-codes.py` now catalogues the `input.*` prefix as well.
  Nothing is extracted, created, cached, logged or written anywhere.
- **Protected extraction planning, without any filesystem (KRX-05, planning
  half).** `openkrx_core::extract::plan` turns an archive inventory into an
  `ExtractionPlan`: the files an extraction would create, decided before
  anything is created. It is a pure function of the inventory — no
  filesystem, clock, process or network access, allocation proportional to
  the entry count, and the same plan for the same inventory every time. A
  plan holds no `PathBuf`, no absolute path and no platform separator; each
  item carries the entry index, the destination path as components, the
  declared size and the decoded size, beside the total bytes and the
  deduplicated implicit parent directories. Five documented limits —
  `max_files` 256, `max_total_bytes` 128 MiB, `max_path_bytes` 1024,
  `max_component_bytes` 255 and `max_depth` 16 — bound the output.
  `ArchiveEntry` now reports the central directory's `version made by` and
  `external file attributes`, and derives `EntryKind` from them, which is
  how a symbolic link, a device node and a directory are recognised: a link
  or a special file rejects the plan rather than being written, and a
  directory marker produces no item, so an empty directory is never
  materialised. Destination components are validated against the union of
  the three target platforms' rules, and two outputs a filesystem could not
  keep apart — equal after NFC normalisation and case folding, or one a
  directory prefix of the other — are refused, never renamed or skipped, as
  is any failure: a rejection rejects the whole plan. Eighteen new
  `extract.*` codes are catalogued in
  [docs/codes.md](docs/codes.md#extraction-planning-codes), the rules in
  [docs/architecture.md](docs/architecture.md#extraction-planning), and the
  threat-model rows in
  [SECURITY.md](SECURITY.md#threat-model-mapping-extraction-planning-layer).
  `unicode-normalization` (MIT OR Apache-2.0) is a new dependency of the
  core crate, for NFC alone.
- **Protected filesystem output and the `extract` command (KRX-05, output
  half).** `openkrx extract <FILE|-> --into <DIR> [--json]` writes what the
  planner decided into a directory the caller names, and is the only code in
  openKRX that writes anywhere. The destination must already exist, must be
  a directory, and must not be a symbolic link or a Windows reparse point —
  `extract` never creates it — and it need not be empty. **Nothing is ever
  overwritten:** a planned path that exists in any form, or an existing
  ancestor inside the destination that is a link rather than a real
  directory, refuses the whole extraction before a byte is written. Files are
  created with `create_new` and directories one level at a time with
  `create_dir`, each re-checked after creation; no permission bit and no
  timestamp is copied from the package. A `.openkrx-extract.partial` marker
  exists for the length of a run, so an interrupted one is detectable and the
  next run into that destination is refused rather than mixed into it. Any
  failure after the first write removes every file, directory and marker
  **this run created**, newest first, and never anything that was already
  there; the report carries `removed` and `left_in_place` counts. A new exit
  status, `9` (`output`), and eight `output.*` codes are catalogued in
  [docs/codes.md](docs/codes.md#output-codes), the policy and the race
  assumptions in
  [docs/architecture.md](docs/architecture.md#extraction-output), and the
  threat-model rows and residual risks in
  [SECURITY.md](SECURITY.md#threat-model-mapping-extraction-output-layer).
  `capabilities().operations` gains `"extract"`. The successful JSON `data`
  is `{files_written, directories_created, bytes_written, items[],
  marker_removed}`, and a failed `extract` response gains a `cleanup` object;
  both are additive, so `schema_version` stays `1`. No runtime dependency was
  added. Extracting a file is not a statement that it is authentic or safe to
  open, and a crash still leaves partial output behind the marker.
- **The documentation set and its maintenance gate.**
  [docs/index.md](docs/index.md) describes every document and carries a
  maintenance map saying which documents each kind of change must update in
  the same pull request. [docs/architecture.md](docs/architecture.md) is
  the canonical reference; [docs/codes.md](docs/codes.md) catalogues every
  stable code with its meaning, numeric fields and asserting test;
  [docs/conformance.md](docs/conformance.md) maps every profile rule to its
  implementation, outcome and test; [docs/research.md](docs/research.md)
  records the searches, the design decisions and the evidence gaps.
  `scripts/check-codes.py` fails the build when a code exists in the crate
  sources but not in the catalogue, or the other way round.
- **Fuzz targets for both readers, and a bounded CI lane.** A `cargo-fuzz`
  package in [fuzz/](fuzz/README.md), outside the root workspace so its
  nightly-only dependencies stay out of the dependency policy, the coverage
  run and the MSRV check. `inventory` drives
  `openkrx_core::archive::inventory` with `Limits::DEFAULT` and re-decodes
  every accepted entry through `entry_bytes`; `xml_metadata` drives
  `openkrx_core::metadata::parse` with `MetadataLimits::DEFAULT`. Neither
  asserts anything about the result: the property is that the call returns on
  any byte string. The `Fuzz (build only)` job builds both on nightly and runs
  each for 30 seconds per CI run, a 60-second budget chosen to catch a
  shallow regression without turning every CI run into a campaign; a
  passing lane is explicitly not evidence that a reader is fuzz-clean. No
  corpus is committed, `fuzz/regressions/<target>/` is reserved for minimised
  fuzzer-generated artifacts, and the truncation and single-byte mutation
  sweeps remain the exhaustive compensating control.
- **A golden output contract.** `tests/golden/` pins the exact `stdout`,
  `stderr` and exit status of 36 command runs: `capabilities` in both modes,
  `inspect`, `list` and `validate-structure` in both modes over five
  fixtures, and `extract` into a fresh directory and into one it refuses
  under the no-clobber rule — exit statuses 0, 3, 4, 6, 8 and 9 between them.
  `scripts/golden.py check|update` compares and rewrites them byte for byte,
  with no normalisation: the JSON envelope's key order is fixed by its Rust
  structs, and a case fails rather than masks if any output ever carries the
  destination it was given. The five deterministic packages under
  `tests/fixtures/golden/` are the first committed fixtures; they are written
  by `crates/openkrx-core/examples/golden_fixtures.rs` from the same
  synthetic writer, recorded in `tests/fixtures/README.md`, and
  `scripts/golden.py verify-fixtures` holds them byte-identical to their
  generator. The `Golden output contract` CI job runs both against a release
  build; `scripts/check.sh` is unchanged, so local runs stay fast.

- **Property-based round-trip tests for the writer and the reader.**
  `crates/openkrx-core/tests/property_round_trip.rs` generates whole
  `PackageSpec` values — metadata with both M4 enumerations, every optional
  header element and marker block present or absent, zero to eight
  attachments whose file names span the accepted component alphabet including
  precomposed Hungarian letters, attachment sizes from nothing to just past
  `Limits::RATIO_GRACE_BYTES`, and any MS-DOS timestamp — and asserts five
  properties over them: a written package reads back with byte-identical
  attachment bytes, the documented normalised document and no failing
  structural check; one request writes the same bytes twice; a single-byte
  mutation is refused or read back inside every ceiling, and never panics; a
  request over one documented ceiling is refused with that ceiling's
  `create.over_limit.*` code; and `extract::plan` accepts every name the
  writer writes. `proptest` is a dev-dependency of `openkrx-core` alone, with
  default features off, so no shipped binary carries it. Each property runs
  64 cases by default, overridable with `PROPTEST_CASES`; shrunk failing
  seeds persist under `crates/openkrx-core/proptest-regressions/` and are
  committed. No library or command behaviour changed.

### Changed

- **`capabilities` reports `stage: "reader-writer"` and names `create`.**
  `capabilities().operations` is now `["inspect", "list",
  "validate-structure", "extract", "create"]`. A consumer branching on the
  operation list sees the new operation; a consumer branching on `stage`
  sees a new word. The `capabilities` goldens changed with it.
- **Two fields may appear in a failed envelope's `error` object.** `field`
  is the JSON Pointer of the manifest field a `create` refusal concerns, and
  `attachment_index` the position in the manifest's `attachments` array.
  Both are absent from every other command's diagnostics, and neither
  carries a value from the manifest, so `schema_version` stays `1`: adding a
  field does not raise it.
- **The four `output.*` codes both writing commands reach now carry a
  sentence per command.** `output.destination_missing`,
  `.destination_not_a_directory`, `.destination_symlink` and `.exists` are
  produced by `extract` and by `create`, and are fixed by different actions,
  so each names the argument the caller actually typed. `extract`'s sentences
  are unchanged to the byte; `create`'s name `--out`. The codes, their exit
  status and every golden of a reading or extracting command are unchanged.

- Archive inventory internals: the end-of-central-directory record is read
  as its 22 fixed bytes, proven present before parsing begins, so locating
  the record can no longer produce a truncation error; the 64 KiB inflate
  output buffer is allocated only for a deflate entry, so a stored entry
  allocates nothing; and `ArchiveInventory::entry_bytes` now documents that
  its decoded budget is per call, capped at `max_entry_decoded_bytes` each
  time and never charged against `max_total_decoded_bytes`. No limit value,
  public signature or accepted archive changes.
- **The extraction planner refuses two shapes it previously planned.** A
  symbolic link declared by an entry whose host system is 19 (OS X) is now
  refused as `extract.unsupported.link`, like one from host 3: both are the
  host systems APPNOTE records as storing `st_mode` in the high external
  attribute bits, so reading only host 3 left a link written on macOS
  classified as `Unknown` and planned as an ordinary file holding its target
  text. No other host is assumed to carry a mode. A destination component
  holding one of `*`, `?`, `<`, `>`, `|` or `"`, none of which a Windows
  filesystem accepts in a name, is now refused as
  `extract.unsafe_path.reserved_character`, and one starting with a space —
  which Windows Explorer strips, as it does a trailing one — as
  `extract.unsafe_path.leading_space`. Those are the only two new codes; no
  existing code was renamed or removed. The entry-kind table and path rules
  in [docs/architecture.md](docs/architecture.md#extraction-planning), the
  catalogue in [docs/codes.md](docs/codes.md#extraction-planning-codes) and
  the extraction rows and residual risks in
  [SECURITY.md](SECURITY.md#threat-model-mapping-extraction-planning-layer)
  record all three decisions. Nothing writes: the planner is still a pure
  function no command reaches.
- **Both crate descriptions, and the documents that still described the
  repository as a scaffold.** `openkrx-core` is described as bounded KRX
  package reading, structural checks and extraction planning, and
  `openkrx-cli` as `inspect`, `list`, `validate-structure` and `extract`;
  both keep `publish = false`. [CONTRIBUTING.md](CONTRIBUTING.md),
  [SECURITY.md](SECURITY.md) and
  [docs/releasing.md](docs/releasing.md) state the implemented surface and
  its boundaries instead, and [README.md](README.md),
  [AGENTS.md](AGENTS.md), [docs/index.md](docs/index.md),
  [docs/roadmap.md](docs/roadmap.md),
  [docs/work-packages.md](docs/work-packages.md) and
  [docs/research.md](docs/research.md) were read against the code and each
  other: the stale claims that nothing is extracted, that
  `capabilities().operations` is empty, that `scripts/check-codes.py` covers
  three code prefixes, and that no filesystem code exists are gone. No
  behaviour, dependency or lockfile entry changes.
- `miniz_oxide` moved from 0.8.9 to 0.9.1, with the root `Cargo.lock` and
  `fuzz/Cargo.lock` refreshed together so both resolve the same version;
  the inflate API in use, the licence and the MSRV are unchanged.

### Removed

- The `archive.truncated.eocd` stable code and its `Structure::Eocd`
  variant. The code was unreachable by construction and asserted by no test;
  an image with no usable end record is reported as
  `archive.malformed.eocd_missing`, as before. `Structure` is
  `#[non_exhaustive]`, so no consumer match was exhaustive over it.

### Fixed

- **Metadata and profile precision.** A repeated `MELLEKLETEK` under one
  `EXPEDIALAS` is now refused as `metadata.malformed.duplicate_element`, like
  every sibling field, instead of being merged into the first list and
  surfacing later as a count mismatch. A first archive entry named exactly
  `mimetype` is the entry the marker check inspects, even when another entry's
  last path segment is also `mimetype`; a marker only under a directory prefix
  stays an unresolved A19 observation. `max_text_bytes` now covers attribute
  values as well as element text, and each event's raw length is checked
  against the remaining headroom before the event is unescaped, so the copy
  materialised from one event never exceeds that bound; the raw event stays in
  the reader's buffer, bounded by `max_document_bytes`. `Metadata`, `StructureReport`,
  `Observations` and `AttachmentResolution` document that their `Debug`
  representation carries package content and must not be logged, and
  `RuleId::A20`, `A21`, `A22` and `M15` are documented as reserved identifiers
  for rules no check asserts.
