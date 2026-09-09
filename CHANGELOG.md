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

- An unpublished Rust workspace, `openkrx-core` and `openkrx-cli`, on
  edition 2024 with a minimum supported Rust version of 1.88. The
  executable offers `--help`, `--version` and `capabilities [--json]`; the
  JSON response is one object carrying `schema_version`, `ok`, `command`,
  `data` and `verified: false`, and its `operations` list is empty because
  no package operation exists. Development checks, CI, and the
  contribution, security and fixture policies were established alongside
  it.
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
  name longer than 200 characters. Two stable codes, `input.unreadable` and
  `input.over_limit.archive_bytes`, were added, and
  `scripts/check-codes.py` now catalogues the `input.*` prefix as well.
  Nothing is extracted, created, cached, logged or written anywhere.
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
