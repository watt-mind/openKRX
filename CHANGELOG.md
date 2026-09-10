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

- **A differential inflate test against an independent decoder.**
  `crates/openkrx-core/tests/differential_inflate.rs` decodes every stream the
  repository can produce a second time with `zune-inflate` — a port of
  libdeflate, and a dev-dependency of `openkrx-core` alone with default
  features off — and asserts it agrees with the reader byte for byte. The
  corpus is every committed `.krx` fixture (which is also the package seed
  corpus `fuzz/seed.py` derives, reproduced in Rust so no interpreter is
  needed), every retained fuzzing regression, packages the synthetic writer
  builds, every deflate level from 0 to 10 so all three block types are
  reached, and two properties over generated payloads. Where the reader refused
  a package over `max_entry_decoded_bytes`, `max_total_decoded_bytes` or
  `max_compression_ratio`, the reference decoder — running under none of
  openKRX's limits — is shown to cross that same ceiling, so a refusal is
  demonstrably the limit rather than one decoder disagreeing with the other.
  No reader behaviour changes and no production dependency is added; the
  residual risk in `docs/roadmap.md` is narrowed rather than retired, because
  the reference decoder is itself unverified.

- **A JSON Schema for the `--json` envelope, checked against the goldens.**
  `docs/schema/openkrx-envelope.v1.schema.json` is the machine-readable form
  of the command contract for `schema_version` 1: one draft 2020-12 schema
  with a `$defs` entry for the failure envelope and for each of the seven
  commands' success `data`, a pattern over the eight stable-code heads, and
  the diagnostic categories. Every documented field is typed and `required`
  where the contract says so, and every report object keeps
  `additionalProperties` open, because adding a field does not raise
  `schema_version`; `error` and `cleanup` are closed instead, so that the
  privacy rule — a diagnostic carries a code, a category, an entry index, a
  manifest pointer and counts, and never a path, an entry name or a value an
  author wrote — is something a validator can check. `scripts/check-schema.py`
  validates all 26 JSON goldens plus six failure envelopes it renders by
  running the executable over synthetic inputs, and reports the first
  violation with the case name and a JSON pointer; CI runs it in the
  `Golden output contract` job with `jsonschema==4.23.0`, and
  `scripts/check.sh` runs it when that package is importable and prints a
  skip line otherwise. Four unit tests in
  `crates/openkrx-cli/src/render/json.rs` read the schema back and assert that
  the `schema_version`, the commands, the code heads and the categories it
  names are the ones this build has, so the schema cannot drift silently. No
  envelope, golden or stable code changed, and no Rust dependency was added.
  The schema describes this executable; it is not a conformance claim about
  any external format or service.
- **The portable arm's created-then-refused directory has a test.** It was the
  one branch of the output layer with none: on the portable path `create_dir`
  succeeds and the `symlink_metadata` that reads the result back finds a
  symbolic link someone put there in between, so the run has created a
  directory it must then refuse. `Resolver::directory` now takes a closure
  called in exactly that window — an ordinary parameter that
  `writer::directories` fills with a no-op, in the same shape as the seam
  `extract::run_between` already had, with no `cfg(test)` branch and no
  feature on the shipped path — and
  `a_directory_this_run_created_and_then_refused_is_accounted_for` in
  `crates/openkrx-cli/src/extract/tests.rs` drives it: the refusal carries
  `output.symlink_in_path`, the ledger has recorded the path, and the undo
  pass reaches it, refuses to `remove_dir` what is now a link, and reports it
  as left in place rather than deleting anything of anyone else's. No
  behaviour changed. `SECURITY.md` and `docs/testing.md` record the case, and
  the two comments that said the undo pass "removes" such a directory now say
  what it does instead.
- **Windows *symbolic* links are exercised by CI, not just junctions.** The
  Windows lane already put a directory junction in every position the output
  layer refuses; a symbolic link is a different reparse-point tag behind the
  same `FILE_ATTRIBUTE_REPARSE_POINT` bit, and it is the tag an attacker would
  actually plant. `mod symbolic_links` in `crates/openkrx-cli/tests/extract.rs`,
  `create.rs` and `repack.rs` now runs the same scenarios against one: a
  destination that is a directory symbolic link
  (`output.destination_symlink`), an intermediate directory that is one
  (`output.symlink_in_path`), a leaf target that is a file symbolic link,
  dangling and live (`output.exists`), and for `create` and `repack` a `--out`
  parent that is a directory symbolic link and an `--out` that is a dangling
  file symbolic link. Each case asserts the code and that nothing was written
  through the link. Two helpers beside the junction one,
  `windows_dir_symlink` and `windows_file_symlink`, shell out to `mklink /D`
  and `mklink` through the same metacharacter guard, and skip the case with a
  `SKIPPED <test>:` line on a runner that cannot make a symbolic link — a
  GitHub-hosted `windows-latest` runner is elevated and can. Tests only; no
  production code changed. The residual risk "Windows symbolic links are not
  exercised by CI" is retired in `SECURITY.md`.
- **An informational public-API compatibility report.** A new
  `API compatibility (informational)` job in `ci.yml` runs
  `cargo-semver-checks` for `openkrx-core` on every pull request, comparing
  the branch against the pull request's base commit — the crate is
  unpublished, so a commit stands in for the released baseline the tool
  normally uses — and writes a `PASS`, `BREAK`, `NOT RUN` or `SKIPPED` line
  plus the tool's own output into the job summary. It is a report and not a
  gate: the
  step carries `continue-on-error: true` and the job is not a required check,
  because nothing is published and no downstream build exists that a break
  could break. The comparison is forced to `--release-type minor`, without
  which the unchanged `0.1.0-dev.0` version on both sides would make the tool
  assume a major bump and skip every breaking-change lint. The job skips
  cleanly, installing nothing, when none of `crates/openkrx-core`,
  `Cargo.toml` and `Cargo.lock` changed, and reports `NOT RUN` rather than a
  finding when setup failed before the comparison started.
  `scripts/api-check.sh [base-rev] [crate]` is the same comparison for a
  maintainer, defaulting to `origin/develop` and `openkrx-core`.
  `docs/testing.md` describes both, `docs/releasing.md` adds reviewing the
  report to the release runbook and records why the gate is still deferred,
  and `docs/roadmap.md` carries the remaining work as an engineering item.
- **Shell completions and a manual page ship with the CLI.** `openkrx
  completions <bash|zsh|fish|powershell|elvish>` writes that shell's
  completion script to standard output, and `openkrx man` writes the roff
  manual page for the whole binary — every subcommand is a section inside the
  one page. Both are generated at run time from the same `clap` definition the
  binary dispatches on, by `clap_complete` and `clap_mangen` (MIT OR
  Apache-2.0, new dependencies of the CLI crate only), so neither can describe
  a surface the build in front of you does not have. Both bypass the JSON
  envelope exactly as `skill` does: nothing on stderr and exit `0` on success,
  no `--json` and no `FILE`, and a usage error — an unknown or missing shell
  name included — exits `2` with an empty standard output. Neither is a
  package operation and `capabilities().operations` is unchanged. Every
  release archive now carries `completions/openkrx.{bash,zsh,fish,ps1,elv}`
  and `man/openkrx.1`, written by the packaged binary and checked by the smoke
  job. Their bytes follow the `clap_complete` and `clap_mangen` versions
  rather than openKRX's own contract, so neither has a golden case;
  `crates/openkrx-cli/tests/completions.rs` holds what does matter, deriving
  the subcommand list from the binary's own `--help`. The release job now also
  marks the draft as a pre-release whenever the tag carries a hyphen — the
  SemVer pre-release form, so `v0.1.0-alpha.1` is one and `v0.1.0` is not —
  rather than leaving the flag to the person publishing the draft, who cannot
  undo a stable release once tooling has fetched it. The draft is still created
  by the workflow and still published by a human.

- **Reparse-point confinement is exercised on Windows CI.** The output rules
  that keep `extract`, `create` and `repack` inside the destination the caller
  named were proven only on Linux and macOS, because their tests need a
  symbolic link. A `cfg(windows)` test helper now creates a directory junction
  with `cmd /c mklink /J`, which needs no elevation and no `unsafe` call, and
  the same three cases run on the Windows lane: an ancestor junction inside
  the destination (`output.symlink_in_path`), a junction as the destination or
  the output directory (`output.destination_symlink`), and a junction at the
  planned output path (`output.exists`). A runner without the builtin prints
  `SKIPPED <test>: mklink /J unavailable`, which libtest shows on failure or
  under `--show-output` and which is greppable in a log where a rule that went
  unexercised would otherwise read as a pass. No behaviour changed;
  `docs/testing.md` and `SECURITY.md` record the narrowed skip.

- **Property-based tests for repacking.**
  `crates/openkrx-core/tests/property_repack.rs` asserts over generated
  packages and generated edit lists what `repack_plan.rs` and
  `repack_rejects.rs` pin by example: an empty `Edits` reproducing the package
  byte for byte, an add and the removal of exactly those numbers restoring it,
  every attachment a valid edit did not name reading back byte-identical with
  the result failing no structural check, an edit naming an attachment the
  package does not carry refused with its `repack.invalid.*` code, and planning
  the same package twice deciding the same thing. The `Edits` strategies live
  beside the writer ones in `tests/support/strategies.rs`.

  Writing them surfaced one **documented asymmetry**, now pinned by its own
  property: adding an attachment to a dispatch that carried no `MELLEKLETEK`
  container and removing it again leaves an empty container behind. M7 makes
  the container and `MELLEKLETEK_SZAMA` separate elements, the writer must emit
  one to hold the added reference, and the later removal cannot know the
  original had none. The property asserts where the difference stops — the
  marker and every attachment byte-identical, exactly one empty `MELLEKLETEK`
  more in the document and nothing else, the parsed documents equal apart from
  `attachments_present`, and a second add-and-remove an exact identity. See
  [docs/testing.md](docs/testing.md#the-container-asymmetry).

- **`openkrx repack`: deterministic editing of an existing package (KRX-08).**
  `openkrx repack <FILE|-> --edits <FILE.json> --out <FILE.krx> [--json]`
  reads one package, applies a strictly validated JSON document of edits, and
  writes the result to a file that must not already exist. `--stdout` writes
  the package to standard output instead, with the report on standard error,
  exactly as `create` does. The core half is
  `openkrx_core::repack::{plan, apply}`: `plan` decides the whole edit before
  anything is written and is inspectable — what is preserved, what changes,
  what goes, what arrives and which header fields the edits set — and `apply`
  writes it through `create::package` with the caller's timestamp. Neither
  touches a filesystem, a clock, a process or a network.

  **Every attachment no edit names is preserved byte for byte**, with its
  `MELLEKLET_LEIRASA`, `MENNYISEG` and `MENNYISEGI_EGYSEG`. The numbers,
  locations, declared sizes and `MELLEKLETEK_SZAMA` are re-derived from the
  attachments the result carries, so removing one renumbers the rest. The
  edits document is one object carrying `schema_version` 1, a required
  `timestamp`, an optional `metadata` object taking any subset of the create
  manifest's header fields in the manifest's own spelling, and the optional
  `add`, `replace` and `remove` arrays; a key the schema does not define is
  refused rather than ignored, and `null` on one of the four optional header
  elements removes it. `remove` and `replace` name attachments by their
  number in the package, counted from 1. The schema is documented in
  [docs/architecture.md](docs/architecture.md#repacking-a-package).

  **A package openKRX cannot re-emit is refused rather than repacked into one
  that lost part of it.** Ten `repack.unsupported.*` codes cover a root
  prefix other than `KRX/OCD/`, another metadata file-name spelling, a
  missing or ambiguous document, a marker that is not where the layout puts
  it, an entry the layout has no place for — a signature document (A7) or a
  service-specific one (A11–A16) — elements outside the grammar (A9), a block
  whose presence alone the reader records (M2, M8), more than one dispatch
  block (M7), and a reference or count the writer would derive differently —
  including a dispatch that declares *no* `MELLEKLETEK_SZAMA`, which would
  otherwise gain the element, because the writer derives the count and always
  emits it.
  Such a package is **not damaged**: `inspect`, `list`,
  `validate-structure` and `extract` all still read it, and the diagnostic
  says so. Three `repack.invalid.*` codes cover an edit naming an attachment
  the package does not carry, two edits naming the same one, and a plan
  applied to an inventory it was not made from.

  The output rules are `create`'s: `--out` must not exist in any form and its
  parent must be an existing real directory, so **the package being edited is
  never overwritten and nothing is edited in place**; a failure after the
  file was created removes it again. The result is read back through the
  structural checks before success is reported, and a failing check is a
  defect in openKRX reported as `repack.internal.self_check_failed`.
  `validate-structure` over a repacked package exits 4, citing A19 and, with
  any attachment, M13 — the same definition of success as for `create`.

  Repacking resolves no profile rule. A19 to A22 and M11 to M15 stay
  unresolved, and a repacked package is "structurally consistent with the
  documented layout", never conforming, never signed and never something a
  receiving service has agreed to accept.

- **14 stable codes**: `repack.unsupported.metadata_missing`,
  `.root_prefix`, `.metadata_name`, `.marker`, `.extra_entry`,
  `.unknown_elements`, `.opaque_block`, `.dispatch_count`,
  `.attachment_reference` and `.attachment_entry`, each classifying to exit
  status 7; `repack.invalid.no_such_attachment`, `.duplicate_target` and
  `.inventory_mismatch`, and `repack.internal.self_check_failed`, each
  classifying to 6. The seven `manifest.invalid.*` codes are reported over
  the edits document too. All are catalogued in
  [docs/codes.md](docs/codes.md#repacking-codes).

- **`repack` in `capabilities().operations`**, beside the five operations
  that were already there. The stage stays `reader-writer`.

- **Three more fuzz targets: structural checks, extraction planning and the
  writer round trip.** `structure` runs `profile::check` over an inventory the
  reader accepted, `extract_plan` runs `extract::plan`, and `create_round_trip`
  builds a bounded `PackageSpec` from the fuzzer's bytes with `arbitrary` and
  calls `create::package`. The last two hold an invariant each: every component
  of a produced plan is non-empty, is neither `.` nor `..` and carries no path
  separator, so a caller can join it blindly; and a package the writer produced
  reads back with no failing structural check and with every attachment
  byte-identical, while a refusal reports a `create.*` code
  [docs/codes.md](docs/codes.md) catalogues — the target reads the catalogue
  itself, so an undocumented code fails the lane. The CI job now runs five
  targets for 30 seconds each, a 150-second total budget. A short bounded run
  is not a campaign, and the lane passing is not evidence that any layer is
  fuzz-clean; see [docs/testing.md](docs/testing.md#fuzzing).

- **A fuzzing seed corpus built at run time, and a weekly campaign lane.**
  `fuzz/seed.py` — Python 3 and the standard library only — builds
  `fuzz/corpus/<target>/` from the committed golden fixtures: each `.krx` for
  the three targets that take a whole package, the extracted metadata document
  for `xml_metadata`, and that document concatenated with the payload members
  for `create_round_trip`. Nothing is committed; the seeds are derived before
  a run and `fuzz/corpus/` stays ignored. `--verify` asserts the corpus exists
  and prints the counts. The pull-request lane now seeds before its 30-second
  runs, on an unchanged budget: an unseeded `inventory` run reached 144 edges
  after 20.5 million executions, while the seeded one *starts* at 514 and ends
  at 798 after 2.4 million deeper ones. A new
  `.github/workflows/fuzz.yml` runs every target for 20 minutes weekly, or for
  a dispatched `minutes_per_target`, measures coverage for `inventory`, and
  uploads the corpus, the coverage report and any crash artifact for 14 days.
  Like the mutation lane it is deliberately **not** a required check; see
  [docs/testing.md](docs/testing.md#fuzzing).

- **`openkrx create`: writing one package from a manifest file (KRX-06,
  command half).** `openkrx create --manifest <FILE> --out <FILE.krx>
  [--json]` reads one JSON manifest, reads the local files it names as
  attachments, and writes the bytes `openkrx_core::create::package` produces
  to a file that must not already exist. `--stdout` writes the package to
  standard output instead and puts the report, JSON object included, on
  standard error — a failure included, so stdout carries the package or
  nothing and a caller piping it never receives a diagnostic in its place.
  openKRX reads no clock, so the manifest's `timestamp` is
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

- **Benchmarks at the input caps, and a scaling guard that fails CI.** Three
  `criterion` targets under `crates/openkrx-core/benches/` measure the reader
  (`archive::inventory` over 1, 16 and 64 MiB of stored and of deflated
  entries and over a 256-entry package, `entry_bytes` over all 256, and
  `metadata::parse` at 10 KiB and near `max_text_bytes`), the planner
  (`extract::plan` over 32 and 256 long Unicode names, `profile::check` over
  32 and 254 referenced attachments) and the writer (`create::package` over
  1 MiB and 16 MiB of attachment bytes). Every package is generated in the
  benchmark's own setup and none is committed.
  `crates/openkrx-core/tests/scaling_guard.rs` is the part that fails CI: it
  measures wall time at a small size and at the ceiling and asserts a ratio
  below 32 for `archive::inventory` (4 MiB against 64 MiB, stored, so inflate
  is excluded), `extract::plan` (32 against 256 entries) and `profile::check`
  (32 against 254 attachments) — about twice linear, far below quadratic, so
  it catches a change of shape and not a change of constant factor. It runs
  in about 3.5 seconds in a debug build. `criterion` is a dev-dependency of
  `openkrx-core` alone, with default features off and only
  `cargo_bench_support` enabled, so no shipped binary carries it. Baseline
  numbers, the thresholds and what each target measures are in
  [docs/testing.md](docs/testing.md#benchmarks). No library or command
  behaviour changed.

### Changed

- **Documentation describes fuzzing, the extraction guarantees and the
  required checks as they are on `develop` (KRX-17).** Wording only, no
  behaviour change: the five fuzz targets, the weekly campaign over a
  cumulative corpus and the retained-regression replay replace the
  "two targets, smoke lane only" description in `docs/architecture.md`,
  `docs/roadmap.md`, `docs/index.md`, `docs/testing.md` and `SECURITY.md`;
  the extraction residual risks now name the portable arm and the mount a
  component walk cannot refuse; and `docs/testing.md` lists
  `scripts/check-schema.py` among the checks `scripts/check.sh` runs and
  carries the current stable-code head list.
- **Extraction resolves and undoes through the destination descriptor on every
  Unix target, not Linux alone (KRX-15).** The descriptor-relative arm was
  Linux-only, because `openat2(2)` is; it now covers `cfg(unix)`, with
  `crates/openkrx-cli/src/extract/linux_fd.rs` renamed to `unix_fd.rs` and one
  `cfg` inside it. Where `openat2` is available the kernel still resolves each
  path under `RESOLVE_BENEATH`, `RESOLVE_NO_SYMLINKS` and
  `RESOLVE_NO_MAGICLINKS`. On every other Unix target — macOS among them —
  the module walks the components itself: one
  `openat(parent, component, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)`
  per component, each relative to the descriptor the step before it returned,
  so a component that is a symbolic link when it is met is refused and one
  swapped after it has been opened can no longer redirect the rest. A
  component the kernel reports as `ENOTDIR` is asked about once, on the error
  path, with an `fstatat` that does not follow links, so a planted link is
  `output.symlink_in_path` on every platform rather than whichever code that
  kernel happened to choose. The walk cannot refuse a mount planted at a
  component, which `RESOLVE_BENEATH` does; that difference is stated in
  `docs/architecture.md` and `SECURITY.md`. This closes the check-to-create
  race on Linux and on Unix targets with `O_NOFOLLOW` directory walks;
  Windows remains on the portable arm, unchanged.
- **The undo pass after a failed write removes through that descriptor too.**
  The ledger in `crates/openkrx-cli/src/extract/cleanup.rs` used to hold
  joined absolute paths and remove them by name. It now records a path's
  components relative to the destination and hands them back to the same
  resolver the creation went through: `unlinkat` for a file and the marker,
  `unlinkat(AT_REMOVEDIR)` for a directory this run created, both beneath the
  destination descriptor on Unix, and `remove_file`/`remove_dir` by name on
  Windows. The removal therefore reaches the directory this run actually
  wrote into rather than whatever now answers to the destination's name: a
  new unit test renames the destination away mid-run, leaves a symbolic link
  to a decoy in its place, and asserts that this run's marker is removed from
  the real directory while the decoy's identically named marker and directory
  are untouched. Nothing is recursive, nothing pre-existing is recorded or
  removed, and a directory that is no longer a directory is still counted
  `left_in_place`. `create` and `repack`, which write one file at a path the
  caller names and hold no destination descriptor, keep removing by name
  through the same ledger.
- **`path_resolution_fallback` keeps its meaning and its values.** It stays
  `true` only where this run asked a Linux kernel for `openat2` resolution and
  could not have it, and `false` wherever the descriptor arm ran — the
  kernel's resolution or the walk — and on a platform with no stronger mode to
  ask for. The golden output contract is unchanged and stays deterministic on
  all three runners.
- **`rustix` moves from a Linux-only to a Unix-only dependency of the CLI
  crate.** Declared under `[target.'cfg(unix)'.dependencies]`, still with
  `default-features = false` and the `fs` and `std` features. On Linux nothing
  changes: the `linux_raw` backend links no C library and brings `bitflags`
  and `linux-raw-sys`. Other Unix targets use its `libc` backend, which adds
  `libc` and `errno` for those targets alone; no Windows build sees it. The
  workspace still forbids `unsafe`, and no `libc` call is made by openKRX
  itself.
- **The race unit tests run on the macOS lane.**
  `crates/openkrx-cli/src/extract/tests.rs` moves from
  `cfg(all(test, target_os = "linux"))` to `cfg(all(test, unix))`, so the
  ancestor swap, the taken leaf, the replaced destination and the
  non-directory component are exercised against the walk as well as against
  `openat2`. The two tests that drive the `openat2` probe stay Linux-only, and
  a new one asserts that a target resolving by walking reports no fallback.
- **Every workflow artifact now sets an explicit retention, and the required
  check list is reconciled with branch protection (PC-04).** The one-day
  baseline applies to the run-scoped intermediates: the `fuzz-artifacts` crash
  upload in `ci.yml`, whose durable copy is `fuzz/regressions/`, and the
  `release-notes`, `archive-<target>` and `checksums` uploads in `release.yml`,
  whose durable home is the draft release. `fuzz.yml`'s `fuzz-campaign` and
  `mutants.yml`'s `mutants-out` stay at 14 days as a documented exception owned
  by the repository maintainer — both lanes are weekly, so a run's evidence has
  to outlive the next scheduled run, the campaign artifact is the only
  surviving copy of the corpus when a campaign fails, and the mutants report is
  what the per-crate floors are argued from. The rationale is repeated as a
  comment beside each setting and tabulated under "Artifact retention" in
  `docs/releasing.md`. Alongside it, `.factory.yaml`'s
  `merge_ci.required_checks` gained `Fuzz (build only)` and `Commit hygiene`,
  the two CI-workflow contexts branch protection requires that the list was
  missing; the `security.yml` contexts stay out of the list because
  `merge_ci.workflow` scopes it to the CI workflow, and `docs/releasing.md`
  now describes the required set by reference instead of naming a count.

- **Four review findings settled in the documentation (KRX-16).**
  `docs/architecture.md` now names the fourth place a failed write to stdout
  or stderr goes unreported — `clap`'s own help, version and usage output,
  whose `io::Result` `main.rs` drops like the other three — instead of listing
  three and saying "every byte". The `Repack it` example in
  `crates/openkrx-core/src/lib.rs` reads the repacked image back and asserts
  the new `consignment_id`, rather than only that the bytes differ. The
  threat-model rows in `SECURITY.md` qualify each Windows-only test with the
  module it lives in (`junctions::`, `symbolic_links::`), because some of them
  share a bare name with the Unix test of the same rule. `AGENTS.md` gains a
  **Referencing tickets** rule: a pull request or commit names the public
  work-package key and never carries a `Fixes` line, because the
  execution-queue identifier is private — so reviewers stop reading its
  absence as an omission. No behaviour changed.
- **The weekly fuzz campaign is cumulative, and retained crashes are replayed
  on every push (KRX-11).** `.github/workflows/fuzz.yml` used to reseed from
  the fixtures every week and throw the result away when the runner shut down.
  It now restores each target's `fuzz/corpus/<target>/` from the previous
  campaign's `actions/cache` entry, seeds on top — `fuzz/seed.py` overwrites
  only its own files, so a restored corpus is left intact — fuzzes for the
  budget, then minimises with `cargo +nightly fuzz cmin` and saves that
  minimised directory as the campaign's new entry, so week n+1 starts from
  everything week n reached. The cache key is
  `fuzz-corpus-<generation>-<target>-<ISO week>-<run id>` with restore keys
  falling back to the latest entry for the target; a maintainer discards the
  accumulation by bumping `CORPUS_CACHE_VERSION` in the workflow. The job
  summary now reports inputs and bytes per target before and after, and the
  `fuzz-campaign` artifact still carries the corpus, the crash artifacts and
  the coverage report for 14 days — on a crash it is the only copy, because
  the save steps do not run when a target fails. No corpus is committed:
  nothing about `fuzz/.gitignore` or the fixture policy changed. Separately,
  the `Fuzz (build only)` job in `.github/workflows/ci.yml` gains one step
  that replays every file under `fuzz/regressions/<target>/` through the built
  target with `-runs=0` and fails on a crash, under a five-minute step
  timeout, so a fixed crash cannot come back unnoticed. Every regressions
  directory is empty today — no crash has ever been found — and a target
  without retained inputs is skipped with a line saying so, rather than run
  against its `.gitkeep`. The lockfile gate and the 30-second per-target smoke
  budget are unchanged. `docs/testing.md`, `docs/releasing.md` (gate 2 and the
  workflow table), `fuzz/README.md` and `fuzz/regressions/README.md` record
  the corpus lifecycle, how to download a campaign's corpus and how to reset
  it.

- **Every push commit keeps its own Security run (PC-05).** The Security
  workflow grouped push runs by `github.ref` and cancelled in progress
  unconditionally, so a second push to `develop` erased the first commit's
  secret-scan, workflow-lint and CodeQL result. The concurrency group is now
  `security-<event>-<pr number or sha>` with
  `cancel-in-progress: ${{ github.event_name == 'pull_request' }}`, matching
  ci.yml: push runs are per commit and never cancelled, while a new revision
  of a pull request still supersedes the previous one. Triggers, permissions,
  jobs and action pins are unchanged.

- **The core crate's documentation carries a compiled example per layer
  (KRX-13).** `crates/openkrx-core/src/lib.rs` gains an `# Examples` section
  with five doctests: writing a package through the public `draft` and
  `create` surface, reading it back through `archive::inventory` and
  `metadata::parse`, running `profile::check` over it, planning an extraction,
  and repacking it. Each is self-contained and each runs entirely in memory —
  no committed fixture, no temporary directory and not the test-only
  `synthetic-writer` feature — because no layer of the crate touches a
  filesystem. `cargo test --doc -p openkrx-core` compiles and runs them, so a
  reader who copies one gets code that is known to build. No behaviour
  changed, no public item was added or removed, and no golden case moved.
  The documentation follow-ups it settles: `docs/research.md` now records why
  `clap_complete` and `clap_mangen` were chosen over committed documents, and
  `docs/architecture.md` states the stance every command shares on a failed
  write to standard output or standard error — the result is dropped, so a
  closed pipe is not a diagnostic, and a successful exit status therefore
  means the operation succeeded rather than that every byte reached the
  consumer, which matters for `create --stdout` and `repack --stdout`.

- **`extract` resolves every destination path beneath one directory
  descriptor on Linux (KRX-09).** After preflight the destination is opened
  once and every directory and file the run creates is resolved by the kernel
  relative to that descriptor with `openat2(2)` under `RESOLVE_BENEATH`,
  `RESOLVE_NO_SYMLINKS` and `RESOLVE_NO_MAGICLINKS`; only a resolved parent
  descriptor and one name component are ever handed to `mkdirat` or `openat`,
  so no absolute path is resolved again after preflight. A path component
  replaced between the check and the creation that follows is refused by the
  kernel rather than followed, under the codes the path rule already has —
  `output.symlink_in_path` and `output.not_a_directory`. Opening that
  descriptor is itself the last check of the destination: it asks for
  `O_DIRECTORY | O_NOFOLLOW`, and a failure refuses the run rather than
  falling back, because falling back would hand a destination that had just
  been replaced to the code that resolves it by name. Only the `openat2`
  probe selects the portable arm. This closes the
  check-to-create race on Linux kernels with `openat2`; macOS and Windows
  keep the previous check-then-create path, their behaviour and their
  messages are unchanged, and the undo pass after a failed write still
  resolves by path on every platform. No new stable code, and
  `capabilities --json` is unchanged.
- **The `extract` JSON result gains `path_resolution_fallback`.** A `bool`,
  present in every successful `extract` report on every platform. It is
  `true` only where openKRX asked the kernel for the stronger resolution and
  could not have it — a kernel before 5.6 answering `ENOSYS`, a seccomp
  filter answering `EPERM`, a missing resolve flag answering `EINVAL` — in
  which case the run takes the portable path once and the human report gains
  a line saying so. It is `false` both when that resolution was used and on a
  platform where there is no stronger mode to ask for, so the value is the
  same on all three runners and the golden output contract pins it;
  `tests/golden/extract-consistent-json/stdout` is the one golden that moves.
  Adding a field does not raise `schema_version`, which stays `1`.
- **`rustix` is a new dependency of the CLI crate, on Linux only.**
  Declared under `[target.'cfg(target_os = "linux")'.dependencies]` with
  `default-features = false` and the `fs` and `std` features, so its
  `linux_raw` backend links no C library and it brings only `bitflags` and
  `linux-raw-sys`. It is the `openat2` call surface, used from one module,
  and it is there because the workspace forbids `unsafe`; the rationale is in
  [docs/research.md](docs/research.md). Its licence,
  `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT`, is accepted by
  `deny.toml` on the MIT arm.
- **First-release readiness is assessed against the six gates (KRX-07,
  openKRX side).** `docs/releasing.md` gains a dated "Readiness status"
  section: one row per gate with a status, public evidence links and what
  would change it. Gates 4 (fixtures, licences, privacy, documentation,
  changelog, vulnerability reporting) and 5 (the separate release change) are
  met; gate 2 is partially met, because the extraction check-to-create race
  and a Windows *symbolic* link are stated rather than tested; gate 3 is met
  per commit and is re-checked on the release commit itself; gates 1 and 6
  are not met. Gate 1 is not met because **independent conformance evidence
  is absent** — nine of thirty-seven rules stay unresolved, and the only two
  routes are the aggregate-only private-corpus report and the outreach
  questions in `docs/research.md`, neither of which is code — so a first
  release could only be a pre-release labelled as such. A concrete proposal
  follows: `0.1.0-alpha.1`, the tag `v0.1.0-alpha.1` that the `plan` job
  requires to equal the workspace version, the `[Unreleased]` entries moving
  under one version heading, the six archives, `SHA256SUMS`, the provenance
  attestations and the draft release the held workflow would produce, and the
  runbook steps a human performs. `publish = false` stays and no crates.io
  publish is part of it. `docs/roadmap.md` and `docs/work-packages.md` record
  KRX-07 as in progress with its consumer half pending, and the residual-risk
  list is reconciled with the assessment. Documentation only; no behaviour,
  no output and no workflow changed, and nothing here claims conformance,
  interoperability, a security guarantee or production readiness.

- **Missing documentation now fails the build.** Both crate roots deny
  `missing_docs`; `openkrx-core` also denies
  `rustdoc::broken_intra_doc_links` and `rustdoc::private_intra_doc_links`,
  so a link in the published surface that resolves only to a private item is
  an error rather than a silently broken link. `openkrx-cli` is a binary
  whose every item is private, which `missing_docs` alone can never reach, so
  it denies `clippy::missing_docs_in_private_items` in its place and every
  item it flagged — the argument types, the subcommand list, the reader
  discriminant, the undo ledger's record and every field of the JSON
  envelope and its diagnostic — now carries a doc comment. The crate-level
  documentation of both crates now names where the stable code catalogue, the
  profile evidence and the CLI contract live. Comments and lint attributes
  only: no behaviour changed.

- **The failed JSON envelope may carry `attachment_number`.** `repack` sets
  it when a refusal concerns one attachment of the package being edited: it
  is that attachment's `CSATOLMANY_SZAMA`, counted from 1, and is a different
  thing from `attachment_index`, which is a position in a manifest or edits
  array. Adding a field does not raise `schema_version`, which stays `1`; a
  consumer ignores fields it does not recognise.

- **`manifest.invalid.unknown_field` now lists the keys the schema defines.**
  The key a caller wrote is still never echoed — it is text they wrote — but
  the sentence names the key set of **the object the diagnostic points at**,
  so a misspelling can be found without reading the schema: a typo inside
  `metadata` is answered with `metadata`'s own fields, not with the
  document's top-level keys. A unit test holds each sentence against the key
  array that defines that object, so the two cannot drift. This applies to
  `create`'s manifest as well as to `repack`'s edits document.

- **`input.unreadable` from `repack` names the file that failed.** `repack`
  opens a package, an edits document and one file per edit, so the failure
  carries the schema path of the one that could not be read: `/` for the
  edits document, `/add/path` or `/replace/path` for a file an edit names,
  and no field for the package. No path is reported, as before.

- **The `output.*` diagnostic sentences `create` and `extract` differ in are
  now keyed for `repack` too**, so a caller who ran `repack` is told about
  `--out` rather than about `--into`, and `output.exists` says that the
  package being repacked is not a legal `--out`.

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

- **A dispatch that carried no `MELLEKLETEK` container no longer gains an
  empty one.** `Dispatch` now retains `attachments_present`, set by the reader
  when the container element was seen, and the writer emits the element only
  when the document carried it or there is a reference to list inside it. M7
  makes the container and `MELLEKLETEK_SZAMA` separate elements, so an empty
  container and no container at all are different documents; until now the
  reader used the fact only to refuse a repeated list and the writer always
  emitted the element, which turned the second form into the first. `repack`
  therefore preserves either shape, and the residual gap recorded against it
  in `docs/architecture.md` and `SECURITY.md` is closed. A caller outside the
  crate sets the flag with `Dispatch::with_attachment_container`;
  `draft::dispatch` carries the container whenever a count is declared, so
  what `create` writes for a manifest is unchanged.

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
