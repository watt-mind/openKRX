# Testing and quality gates

Everything openKRX asserts about a format rule is held by a named test. This
document says which test file holds what, how to run the suite and the
coverage gate, what the sweeps and the fuzz targets each guarantee, and the
rules for fixtures and for any private corpus.

## Test layout

Almost every test is an integration test, because the contract worth testing
is the public one: a byte slice goes in, a typed value or a stable code comes
out. The exceptions are the doctests in the public API documentation and five
`#[cfg(test)]` modules.

The doctests are compiled and run by `cargo test --doc`, which
`cargo test --workspace` includes. `crates/openkrx-core/src/lib.rs` carries one
worked example per layer — writing a package, reading it back, running the
structural checks, planning an extraction, and repacking it — so a reader of
the crate documentation can copy an example that is known to build. Each one
builds its own package through the public `draft` and `create` surface, so
none needs a committed fixture or the test-only `synthetic-writer` feature,
and each runs entirely in memory, because no layer of the core crate touches a
filesystem.

Of the `#[cfg(test)]` modules, the one in
`crates/openkrx-core/src/archive/inflate.rs` pins the CRC-32 implementation
against its published check value, the empty input, and chunk ordering — an
internal helper with no public surface to exercise it through. The ones in
`crates/openkrx-core/src/archive/kind.rs`,
`crates/openkrx-core/src/extract/paths.rs` and
`crates/openkrx-core/src/extract/collisions.rs` cover the host-system kind
mapping, the path-component classes and the path folding directly: two
component classes — `..` and a C0 control — are already impossible in an
accepted inventory, so the only way to hold the planner's own defence in
depth is to call it. The one in `crates/openkrx-core/src/create/mod.rs`
holds the entry-count ceiling a non-ZIP64 end record imposes: writing 65 536
entries through the public writer would cost seconds of compression to prove
a `u16`, so the ceiling check is called directly. The one in
`crates/openkrx-cli/src/exit/tests.rs` holds one test per exit-status category
and
reads every code out of `docs/codes.md` to assert that each classifies, with
no head list of its own: a subprocess can
reach only the codes an archive can be built to produce, and
the contract covers every code the crates define. The one in
`crates/openkrx-cli/src/extract/tests.rs`, behind `cfg(unix)`,
holds the check-to-create race and the undo pass that follows a lost one:
what they test happens *inside* one run, between the moment preflight
accepted a destination and the moment the first file is written, and no
subprocess can be interrupted there. They run on the Linux and the macOS
lane, which is how the component walk is exercised; the two that drive the
`openat2` probe stay behind `cfg(target_os = "linux")`, because no other
target has a stronger resolution to fall back from.

| File | Covers |
| --- | --- |
| `crates/openkrx-core/tests/archive_inventory.rs` | Archives the inventory accepts and the observations it reports: central-directory ordering, declared metadata beside counted decoded size, the UTF-8 flag reported independently of whether the name decodes, empty and zero-length and directory entries, `entry_bytes` re-decoding one entry, data descriptors with and without their signature, tolerated comments and extra fields, that `Limits::DEFAULT` is the documented table, and that `Display` prints codes and numbers but never an entry name. |
| `crates/openkrx-core/tests/archive_rejects_structure.rs` | Archives that contradict themselves: a missing, duplicated or displaced end record, a wrong declared record count, a missing signature, local headers and data descriptors disagreeing with their record, prefix, trailing, unclaimed and overlapping bytes, CRC and declared-size mismatches, corrupt and truncated deflate streams, ZIP64 markers and multi-disk end records. Holds the truncation sweep and the single-byte mutation sweep. |
| `crates/openkrx-core/tests/archive_rejects_input.rs` | Input the limits and name rules refuse: every limit at its boundary (below, at, above), deflate bombs stopped by the ratio, per-entry and total ceilings, the ratio grace window, every unsafe-name class, byte-identical and case-folded name collisions, unsupported methods, encryption and patched-data flags, ZIP64 records, and a record on another disk. |
| `crates/openkrx-core/tests/extract_plan.rs` | Plans the extraction planner produces: a KRX-shaped package planned in inventory order, declared beside counted sizes, directory markers producing no item while their children produce deduplicated sorted parent directories, an empty directory never materialised, a plan free of separators and roots, determinism over one inventory, every `EntryKind` read from the central-directory fields, that `ExtractLimits::DEFAULT` is the documented table, and the fixture and name-mutation sweeps that hold "no panic on any input". |
| `crates/openkrx-core/tests/extract_rejects.rs` | Everything the planner refuses, each by its stable code: symlink and special-file entries, a name that is not UTF-8, every unsafe component class an archive can carry, NFC and case collisions, a file that is also a directory prefix, every extraction limit at its boundary, and the content-free `Display` assertion. |
| `crates/openkrx-core/tests/metadata_parse.rs` | Documents the parser accepts and the facts it reports: prefixed and default namespace bindings, every enumeration token the schema lists, optional header and attachment elements present and absent, the joined attachment path and its edge cases, and the values the sources leave open — an absent `TESZT`, an absent `MELLEKLET_LEIRASA`, a non-numeric or infinite `MERET` — which are recorded rather than rejected. Also pins `MetadataLimits::DEFAULT` and the target-namespace constant. |
| `crates/openkrx-core/tests/metadata_rejects.rs` | Every refused XML feature (DTD, undeclared entity, forbidden character reference, stray processing instruction, non-UTF-8 declaration and non-UTF-8 bytes, unbound prefix, repeated attribute), every grammar violation (wrong root, wrong namespace, missing, repeated and misordered elements, unexpected child, enumeration, integer and boolean values), and every metadata limit at its boundary. Holds its own truncation and mutation sweeps, and the content-free `Display` assertion. |
| `crates/openkrx-core/tests/profile_structure.rs` | The check inventory over synthetic KRX-shaped archives: the canonical layout, all three observed root prefixes, both metadata file-name spellings, a lower-case `Metalayer`, missing and ambiguous metadata, a missing, misplaced, mismatched and prefixed marker, a deflated marker producing no finding, missing, prefix-variant and duplicate references, a shared file name in different payload directories, count agreement and its absence, an omitted schema-required element, a document that does not parse, caller-tightened limits, and that every check is reported exactly once in the documented order. |
| `crates/openkrx-core/tests/create_package.rs` | What the deterministic writer produces: the documented layout and its fixed entry order for zero, one and three attachments, the stored marker beside deflated entries, the UTF-8 flag, host system and file mode on every entry, the document's declaration, prefix, element order and unqualified `KEZELESI_UTASITASOK`, the derived references and count, byte-identical output across runs and its sensitivity to one attachment byte, the caller's timestamp in both headers of every entry, attachment bytes read back unchanged, a document that survives parse and re-write unchanged, and the report a written package produces — no failing check, and exactly A19 and M13 left undecided. |
| `crates/openkrx-core/tests/create_rejects.rs` | Everything the writer refuses, each by its stable code: every unsafe file-name class including the ones only the extraction planner would catch, a reference or a declared count that disagrees with the attachments, attachments with no dispatch block and a document with two, elements the grammar does not define, text XML 1.0 cannot carry and text the reader would trim, every timestamp field out of range, an attachment that compresses far enough for the reader to call it a bomb, and every output ceiling at its boundary with the package still readable under the tightened limits — including the invariant those ceilings exist for, that whatever `package` returns passes the inventory, the structural checks and `entry_bytes` on every attachment. |
| `crates/openkrx-core/tests/repack_plan.rs` | What repacking preserves and what it writes: an empty edit reproducing the package `create` wrote byte for byte, repacking a repacked package changing nothing, every preserved attachment's bytes equal to the input's after a removal renumbered it, a repacked package failing no structural check, header fields set and optional elements set and cleared, an added attachment appended with its reference derived, a replacement changing the bytes and the declared size while keeping the name and description, removal down to a document declaring none, the three edit kinds together renumbering the result, a plan applied twice writing the same bytes, and a repacked package equal to creating the same package from scratch. |
| `crates/openkrx-core/tests/repack_rejects.rs` | Everything repacking refuses, each by its stable code, over packages that differ from a canonical control in exactly one respect: another root prefix, another metadata file-name spelling, no metadata document and two of them, a marker that is missing, displaced or holding something else, an entry the layout has no place for, elements outside the grammar, each block the reader records only the presence of, an unqualified handling-instruction element, two dispatch blocks, a reference or a declared count the writer would derive differently, a reference naming another entry, a payload entry no reference declares, a document that does not parse reporting the parser's own code, an edit naming an attachment number the package does not carry or naming one twice, a plan applied to another package, and a result above the entry ceiling refused while planning. |
| `crates/openkrx-core/tests/property_round_trip.rs` | The five [property-based tests](#property-based-tests) over generated requests: a written package read back with byte-identical attachments, the documented normalised document and no failing check; byte-identical output across two writes; a single-byte mutation refused or read back inside every ceiling; a request over one documented ceiling refused with that ceiling's code; and every written name accepted by the extraction planner. Its strategies live in `crates/openkrx-core/tests/support/strategies.rs`. |
| `crates/openkrx-core/tests/property_repack.rs` | The six [property-based tests](#property-based-tests) over repacking: an empty edit reproducing the package byte for byte, an add and the removal of exactly those numbers restoring it, the one documented asymmetry where a dispatch that carried no `MELLEKLETEK` container gains an empty one and nothing else, every attachment a valid edit did not name reading back byte-identical with no failing structural check, an edit naming an attachment the package does not carry refused with its `repack.invalid.*` code, and planning the same package twice deciding the same thing. Shares `crates/openkrx-core/tests/support/strategies.rs` with the writer properties. |
| `crates/openkrx-core/tests/scaling_guard.rs` | The [scaling guard](#the-scaling-guard): `archive::inventory` at 4 MiB and 64 MiB of stored entries, `extract::plan` at 32 and 256 entries with long Unicode names, and `profile::check` at 32 and 254 referenced attachments, each asserted to cost no more than 32 times as much at the ceiling as at the small size. It measures wall time, so it holds a shape rather than a number. |
| `crates/openkrx-core/tests/metadata_evidence.rs` | The independent-evidence layer: a synthetic re-expression of the *structure* of the two official sample documents (rules M9 and M10), parsed into the documented shape and resolved inside the documented layout. |
| `crates/openkrx-core/src/synthetic/` | Test-only synthetic writers, not tests, behind the non-default `synthetic-writer` feature so that the command-line tests can build the same archives. `mod.rs` builds ZIP images and can emit contradictory headers on purpose; `meta.rs` builds metadata documents from values written from scratch for this repository. `crates/openkrx-core/tests/support/mod.rs` re-exports them under the name the core tests use. |
| `crates/openkrx-core/examples/golden_fixtures.rs` | The generator behind the twelve committed files under `tests/fixtures/golden/`, not a test: five packages — a canonical two-attachment package, the same package without the `KRX/OCD/` prefix, one declaring an attachment the archive does not hold, a truncated image and an over-limit one — and the inputs of the `create` cases: two small attachment files and three manifests, one that writes a package, one carrying a key the schema does not define, and one naming an attachment that is not there — and the two edits documents the `repack` cases apply, one that changes a header field and adds an attachment and one naming an attachment number the package does not carry. Deterministic by construction; see [the golden output contract](#golden-output-contract). |
| `tests/golden/` | The byte-exact output contract: one directory per command run, holding its `cmd`, its exact `stdout` and `stderr` and its exit status, compared by `scripts/golden.py`. |
| `crates/openkrx-cli/tests/contract.rs` | The executable's argument surface by subprocess: the capability envelope's exact shape and its operation list, that the human output states the boundary rather than a verdict, that help names every command and the exit statuses, and that six kinds of argument error each exit 2. |
| `crates/openkrx-cli/tests/reader.rs` | Each reader command on a package that can be read, in both modes: the consistent package, the entry listing and its stable ordering, the declared document beside the observed archive, every check rendered as its own outcome, the two other layouts leaving A19 undecided, `-` on all three commands producing the same report as a file, a name that is not UTF-8 reported as bytes, an invisible character escaped, a long name cut with its remainder counted, and that a successful run writes nothing on stderr. |
| `crates/openkrx-cli/src/extract/tests.rs` | Linux only, in-process: the window between preflight and the first write, driven through a private seam in `extract::run_between` that calls a closure once in exactly that window. An ancestor directory the run created is replaced with a symbolic link to a directory outside the destination and the write is refused with `output.symlink_in_path`, with nothing written through the link; the same ancestor replaced by a file is refused with `output.not_a_directory`; the leaf taken by a directory is refused under the output category. A fourth holds the other window, through the resolver's own chooser: the destination is replaced by a symbolic link after preflight accepted it and before the descriptor is opened, and the run is refused under a path code rather than continuing on the portable arm, with nothing written through the link. Two more hold the report: forcing the `ENOSYS` branch — the probe answering "this kernel has no `openat2`" — still extracts and reports `path_resolution_fallback: true`, and an ordinary run on a kernel that has the call reports `false`. |
| `crates/openkrx-cli/tests/extract.rs` | `extract`, the one command that writes, by subprocess against a destination each test owns and then reads back: a successful extraction with byte-identical payloads and nothing else created, the JSON report and its item order, a target file that already exists, a destination that is missing, a file, or a symbolic link, a marker left by an interrupted run, an entry named like that marker refused under the no-clobber code, a non-empty destination whose contents survive, a planner refusal keeping its own category and naming no path, a malformed and an unreadable package never reaching the destination, the usage errors, and — behind `cfg(unix)` — an ancestor symlink inside the destination, a pre-existing symlink at the leaf, a destination that is itself a symlink, and an injected write failure proving the cleanup pass removes exactly this run's files and leaves everything else. `mod junctions` and `mod symbolic_links`, behind `cfg(windows)`, put a directory junction and then a real Windows symbolic link in each of those three link positions — the leaf twice over, dangling and live — so the reparse-point half of the rule is exercised on the platform that has one. One test asserts that a successful report carries `path_resolution_fallback: false` and that the human report says nothing about path resolution — the same on all three operating systems, which is what lets the golden output contract pin it. |
| `crates/openkrx-cli/tests/create.rs` | `create`, the other command that writes, by subprocess over manifests and attachment files each test writes itself: the two fixed entries of a package with no attachment, three attachments placed and numbered with `file_name` honoured, the round trip through `list`, `inspect`, `validate-structure` (exit 4, never 3) and `extract`, byte-identical output across two runs, `--stdout` producing the same bytes with the report on stderr, one case per manifest defect asserting its code and its schema path, an unknown key refused rather than ignored, a missing manifest and a missing attachment, the file-name classes the writer refuses, a declared count that agrees and one that does not, an output file that already exists, a missing and a non-directory output parent, and — behind `cfg(unix)` — a symbolic-link parent, a dangling link at the output path, and a read-only destination directory reporting `output.io`, with `mod junctions` and `mod symbolic_links` behind `cfg(windows)` putting a directory junction and then a real Windows symbolic link in the two output-link positions this command has. One test holds the `--stdout` failure rule: stdout carries the package or nothing, so a refused run leaves it empty and puts one JSON object on stderr. Every refusal asserts that no package was written, and two canary tests assert that no manifest value reaches either stream. |
| `crates/openkrx-cli/tests/repack.rs` | `repack`, the third command that writes, by subprocess over a package the executable itself created: an attachment no edit names extracted from both packages and compared byte for byte, an empty edit rewriting the source package byte for byte and idempotently, the package being edited left untouched, the JSON report's four attachment lists and header fields, the human report distinguishing the two numberings, a header edit visible through `inspect`, the package read from standard input, `--stdout` producing the same bytes with the report on stderr, a foreign layout refused with exit 7 and a diagnostic that says the package is not damaged, an edit naming an attachment the package does not hold and one naming the same attachment twice, one case per edits-document defect asserting its code and schema path, a missing timestamp and an unknown schema version, a file an edit names that cannot be read, an output that already exists, repacking onto the package being edited, a junction as the output directory and one at the output path, and the same two positions again as a real Windows symbolic link, all behind `cfg(windows)`, an unreadable and a malformed package, the usage errors, and two canary tests asserting that no value an edits document carried reaches either stream. |
| `crates/openkrx-cli/tests/failures.rs` | Every exit-status category end to end: a failing check (3) for `validate-structure` only, malformed and truncated images (6), a ZIP64 extra field (7), an over-limit entry count with its numbers (8), a missing file, a directory and both an over-cap file and an over-cap standard input (5), one stderr line in human mode, and exactly one object on stdout in JSON mode even when the run failed. |
| `crates/openkrx-cli/tests/render_text.rs` | The human renderer's own lines, asserted whole rather than by substring: each attachment block with the entry it resolves to and its observed decoded size, a reference that resolves to nothing, one matched only after a root-prefix adjustment, the metadata entry with its index, the format-marker outcome, the failed and undecided check counts in the summary sentence, a check that does not apply reported as such rather than as undecided, the 200-character cut boundary at the limit and one past it, and the declared attachment count taken from the document rather than from the number of references. |
| `crates/openkrx-cli/tests/diagnostics.rs` | The rest of the diagnostic contract: the one content-free sentence each `output.*` code is given instead of its category's, for a destination that is not a directory, an occupied ancestor, an ancestor symbolic link and a failed write; the cleanup line a refusal reached before any write produces; and that a limit failure keeps its limit and observed numbers whichever command reported it. |
| `crates/openkrx-cli/tests/privacy.rs` | The content-free-diagnostic rule, by canary: a canary path segment, a canary entry name and a canary metadata value are searched for on stderr in every case and on stdout in every failing case, a declared value is shown to reach stdout for `inspect` alone, and `extract` is held to the same rule with its one documented exception — a successful report names the files it created, and still never the destination or a declared value. |
| `crates/openkrx-cli/tests/support/mod.rs` | Subprocess helpers and the synthetic packages the command tests read, not tests: a scratch directory that removes itself, a standard-input runner, three `cfg(windows)` link helpers that shell out to `mklink /J`, `mklink /D` and `mklink` behind one metacharacter guard and print a `SKIPPED <test>:` line if the link cannot be made, and one builder per package shape. |

Each rejection test asserts a **stable code**, not merely that an error
occurred, so one rejection category cannot silently become another.
[SECURITY.md](../SECURITY.md#threat-model-mapping-archive-layer) maps each
required threat-model check to its code prefix and test, and
[conformance.md](conformance.md) maps each profile rule to the test that
holds it.

The command-line tests run the built executable as a subprocess, so they
exercise the real argument parsing, the real streams and the real exit
status rather than an internal function standing in for them. They avoid
platform-specific assumptions — no shell, no Unix path shape, no text-mode
standard input — because CI runs the same tests on Linux, macOS and Windows,
which is the only check on the platform-specific parts of input handling.

A few tests need a platform primitive that is not portable. The `std::os::unix`
symbolic link tests are behind `cfg(unix)`, because the call that makes one is
not portable. The rule they hold is not skipped on Windows: two `cfg(windows)`
modules in `extract.rs`, `create.rs` and `repack.rs` put a Windows reparse
point in each of the same positions, so the Windows half of the rule —
`FILE_ATTRIBUTE_REPARSE_POINT` in `cli::extract::preflight::is_link` — is
exercised by the Windows CI lane rather than held by construction.

`mod junctions` uses a *directory junction*, which `mklink /J` creates with no
privilege at all, so it runs on any runner. `mod symbolic_links` uses a real
Windows *symbolic link* — `mklink /D` for a directory, `mklink` with no flag
for a file — which needs developer mode or an elevated process; a
GitHub-hosted `windows-latest` runner is elevated, so the CI lane runs them. A
junction and a symbolic link are different reparse-point *tags* behind the same
attribute bit, and the second module makes the tag CI exercises the one an
attacker would actually plant. The leaf cases cover both a dangling and a live
target, because a dangling link is the one shape where "the path exists" and
"there is something behind it" disagree, and the live one proves nothing was
written through to the file.

Both helpers shell out to `cmd`, so no `unsafe` call and no new dependency is
needed under `unsafe_code = "forbid"`, and both compose the command through the
same metacharacter guard, which panics rather than let a `&^|<>"` from the
temporary root be read as syntax. If the link cannot be made the helper prints
`SKIPPED <test>: mklink /J unavailable` — or `mklink /D`, or `mklink` — and the
test returns. libtest captures that line and shows it on failure or under
`--show-output`, so it is not loud on a green run; what it buys is a greppable
record naming the case, rather than a rule that went unexercised and read as a
pass. That `SKIPPED <test>:` shape is these helpers' and `diagnostics.rs`'s; it
is the shape a new self-skipping test should follow.

The injected write failure stays behind `cfg(unix)`: Windows ACL inheritance
does not make a directory unwritable through one `set_permissions` call, and
the test in `extract.rs` skips itself on Unix too when the process can write
into a read-only directory anyway — with a lowercase `skipped:` line on
standard error rather than that shape. Everything else in `extract.rs` runs on
all three operating systems.

What the suite does **not** provide evidence about: that a package openKRX
writes is accepted by any real producer or service — the writer's round-trip
tests prove only that openKRX agrees with its own reading of the documents,
and A19–A22 and M11–M15 stay unresolved over a package it wrote itself;
 extraction under a
concurrent writer at the destination,
which is outside the threat model and stated as a residual risk in
[SECURITY.md](../SECURITY.md#residual-risks-of-the-output-layer);
conformance, which cannot be claimed while
[profile.md](profile.md#unresolved-essential-rules) lists unresolved rules;
and agreement with a real service, for which no sample or reference
implementation exists.

## Running the tests and coverage

```sh
bash scripts/check.sh
```

That is the gate. It runs, in order: `cargo fmt --all --check`;
`cargo clippy --workspace --all-targets --locked -- -D warnings`;
`cargo test --workspace --locked`; `cargo doc --workspace --no-deps --locked`
with `RUSTDOCFLAGS="-D warnings"`; `cargo machete`;
`cargo deny --all-features check`; `python3 scripts/check-doc-links.py`;
`python3 scripts/check-codes.py`; `python3 scripts/check-file-length.py`;
markdownlint; and `actionlint`.

To run one part by hand:

```sh
cargo test --workspace --locked
cargo test --workspace --locked profile_structure
cargo test -p openkrx-core --locked -- --nocapture single_byte_mutations
PROPTEST_CASES=1024 cargo test -p openkrx-core --locked --test property_round_trip
```

Coverage:

```sh
cargo llvm-cov --workspace --locked --all-features --summary-only
cargo llvm-cov --workspace --locked --all-features --html
```

CI also holds the minimum supported Rust version: the
`Minimum supported Rust (1.88)` job forces the 1.88 toolchain with a job-level
`RUSTUP_TOOLCHAIN` environment variable — because `rust-toolchain.toml` pins
`channel = "stable"` and a toolchain file otherwise overrides the installed
default — and prints `rustc --version` as evidence before it runs
`cargo check --workspace --all-targets --locked`. Locally the equivalent is
`cargo +1.88 check --workspace --all-targets --locked`.

The workspace line-coverage floor is **90 %**, enforced in CI. It is a floor,
not a target: parser and filesystem work must add meaningful boundary
coverage rather than only raise the aggregate. A test that merely repeats a
trivial implementation detail raises the number without adding evidence and
should not be written.

Three checks are Python, standard library only, and run offline:

| Script | What it enforces |
| --- | --- |
| `scripts/check-doc-links.py` | Every relative Markdown link resolves, and every heading anchor it names exists. External links are never fetched, so the check stays deterministic. |
| `scripts/check-file-length.py` | Tracked Rust files stay under 800 lines, or 1 500 for test files. |
| `scripts/check-codes.py` | Every stable code literal in `crates/*/src/**` appears in [codes.md](codes.md), every code that document lists still exists in the sources, and the heads the catalogue documents are exactly the heads the script's `HEADS` line names. |

`scripts/check-codes.py` is the maintenance gate for the code catalogue. It
fails in both directions and names the offending code and the file that
defines it, so adding a code without cataloguing it, or renaming one and
leaving a stale row behind, both stop the build. Run it alone with
`python3 scripts/check-codes.py`; on success it prints one line naming how
many codes are catalogued.

The head list — `archive`, `extract`, `input`, `metadata`, `output` — is
written once, in the `HEADS` line of that script, and two gates hold the rest
of the tree to it. The script builds its extraction patterns from `HEADS` and
fails when the heads [codes.md](codes.md) documents in backticks are not
exactly those. The CLI catalogue test
`every_catalogued_code_classifies_to_a_category` in
`crates/openkrx-cli/src/exit.rs` fixes no head list at all: it takes every
backticked dotted lower-case token of the catalogue as a code, asserts that
`Category::of_code` classifies each one, and then asserts that the heads it
saw equal the heads it parses from that same `HEADS` line. A new head added
to the script alone leaves the catalogue and the test disagreeing with it, and
a new head documented in the catalogue alone is not extracted and reaches
`of_code` unclassified; either way `bash scripts/check.sh` or
`cargo test -p openkrx-cli` fails, so a head can never be added in one place
only.

## Golden output contract

The Rust tests assert the facts each command reports. They do not pin the
**bytes**, so a renamed JSON field, a reordered key, a reworded human line or
a changed exit status passes every test that does not happen to name it. The
golden contract closes that gap: `tests/golden/<case>/` holds one command's
exact `stdout`, exact `stderr` and `status`, and `scripts/golden.py` compares
them byte for byte.

What it pins, in 52 cases: `capabilities` in both modes; `inspect`, `list`
and `validate-structure` in both modes over all five committed fixtures —
which between them cover a consistent package, an undecided rule (A19), a
failed check, a malformed image and an exceeded limit, so exit statuses 0, 3,
4, 6 and 8 all appear; `extract` in both modes into a fresh directory,
plus a second run into the same directory, which is refused under the
no-clobber rule with status 9; and `create` in both modes over the committed
manifests — a package written into a fresh directory, a second run refused
under the same no-clobber rule (9), a manifest carrying a key the schema does
not define (6) and one naming an attachment that is not there (5); and
`repack` in both modes over a package the case's own `setup` step creates
first — a successful edit that preserves two attachments, adds one and sets
two header fields, an edits document naming an attachment number the package
does not carry (6), an `--out` that already exists because it *is* the package
being edited (9), and the committed no-prefix fixture refused as
`repack.unsupported.root_prefix` (7), which is the one golden that pins what a
caller sees when openKRX will not rewrite someone else's layout.

A case directory holds `cmd`, one line of arguments, and optionally `setup`,
one line run first whose output is discarded — that is how the no-clobber
case gets a destination that is already populated. Two placeholders are
substituted: `{fixture}` becomes the repository-relative
`tests/fixtures/golden`, and `{outdir}` becomes an empty directory the case
owns. **Nothing is normalised.** The JSON envelope is serialised from Rust
structs in declaration order, so its key order is fixed; both renderers are
pure functions of the package; `extract` reports destination-relative
paths and never the destination itself; and `create` and `repack` report
counts, attachment numbers, cited rules and the layout name, and never a
path, a file name or a manifest or edits value. A `create` or `repack`
golden is byte-stable for the same reason the writer is: the manifest or the
edits document carries the timestamp, and openKRX reads no clock.

The only absolute path any case is given is its `{outdir}`, and the runner
*fails* when that name appears on either stream rather than masking it,
because an output that carried it would be a privacy bug in the executable,
not a gap in the script.

`completions` and `man` have no golden case either, and for a different
reason: their bytes are produced by `clap_complete` and `clap_mangen`, so a
recorded script or page would pin the dependency version rather than openKRX's
own contract, and a routine dependency bump would rewrite a golden without
anything about the CLI having changed. What matters about them is held in
`crates/openkrx-cli/tests/completions.rs` instead: every subcommand the
binary's own `--help` lists must appear in each of the five completion scripts
and in the man page, the page must reproduce every exit-status digit, and both
commands must bypass the envelope and exit 2 on a usage error.

`create --stdout` and `repack --stdout` have no golden case, because a golden
compares the two text streams and that mode writes a binary package to one of
them. The subprocess tests cover it instead: for each command, one test
asserts that `--stdout` and `--out` produce byte-identical packages with the
report on stderr, and `create`'s asserts as well that a failed `--stdout` run
leaves stdout empty and puts a single JSON object on stderr.

Run it locally against a release build:

```sh
cargo build --release --locked -p openkrx-cli
python3 scripts/golden.py check --bin target/release/openkrx
python3 scripts/golden.py verify-fixtures
```

It is deliberately **not** part of `bash scripts/check.sh`, which must stay
fast and needs no release binary. CI runs it in the `Golden output contract`
job, which is a required check.

`scripts/golden.py update --bin <path>` rewrites the goldens. A changed
golden is a change to the contract every consumer parses, so it is reviewed
as one: the pull request body must say which command's output changed and
why, and a removed, renamed or redefined envelope field also raises
`schema_version` and is recorded in [CHANGELOG.md](../CHANGELOG.md). Running
`update` to make a red build green, without that justification, is the one
thing this gate exists to prevent.

## Mutation testing

Coverage says a line ran. It does not say a test would notice the line doing
something else. [cargo-mutants](https://mutants.rs/) answers that instead: it
rewrites one expression at a time — `>` to `>=`, `&&` to `||`, a match arm
deleted, a function body replaced by a fixed value — rebuilds, and runs the
suite against each mutated copy. A mutant the suite still passes against is a
**survivor**: behaviour changed and nothing caught it. For a reader whose
safety rests almost entirely on rejection branches, that is the property worth
measuring.

`.github/workflows/mutants.yml` runs it weekly and on manual dispatch, and it
is deliberately **not** a required check. It uploads `mutants.out/` and the
gate summary as the `mutants-out` artifact, kept for 14 days rather than the
one-day baseline — a documented exception, recorded under
[artifact retention](releasing.md#artifact-retention). A full workspace run
rebuilds and reruns the suite once per mutant. The seeding run — the whole
workspace, both crates, all 937 mutants, in one
`cargo mutants --workspace --jobs 2` invocation — took **7 minutes 39 seconds**
of wall time on the machine it was measured on. That is short because most
mutants rebuild incrementally in under a second and this suite runs in about
one; the two mutants that make the
parser loop forever cost `minimum_test_timeout` — 120 seconds each — which is
a quarter of the total on its own. A shared runner is slower, and every
function added makes the run longer, which is why it does not belong in a
pull-request build. A failure there is a prompt to strengthen a test, not a
merge block.

The seeding run scored `openkrx-core` at 567 caught, 16 missed, 84 unviable
and 1 timeout — 97.26 % of the mutants that both compiled and finished — and
`openkrx-cli` at 219 caught, 21 missed, 28 unviable and 1 timeout, 91.25 %.
Those are the numbers `scripts/mutants-floors.txt` was seeded from.

### Running it locally

```sh
cargo install cargo-mutants --locked
cargo mutants --workspace --jobs 2 --output tmp
python3 scripts/mutants_gate.py --dir tmp/mutants.out
```

`--output tmp` puts the report under the already-ignored `tmp/` directory
rather than at the repository root, where it would show up as untracked; the
scheduled workflow omits the flag and uses the default location, which is why
the gate defaults to `--dir mutants.out`.

`cargo mutants` writes a `mutants.out/` directory: a log per mutant,
`caught.txt`, `missed.txt`, `timeout.txt` and `unviable.txt` listings, and
`outcomes.json`, which the gate reads. `--jobs 2` is a memory bound, not a
speed choice: each job builds its own copy of the workspace. Useful narrowing
flags are `-p openkrx-core` or `-p openkrx-cli` for one crate,
`--file crates/openkrx-core/src/archive/eocd.rs` for one file, and
`--shard 1/4` for a quarter of the mutants; `cargo mutants --list --workspace`
counts them without running any.

`.cargo/mutants.toml` holds the configuration and the reason for each entry:
`exclude_globs` drops the test-only synthetic writer, the examples directory
and the separate fuzz package, none of which is shipped behaviour, and
`timeout_multiplier` with `minimum_test_timeout` bound a mutant that loops
forever without turning a slow runner into a false timeout. There is no
function-name exclusion: the `Display` impls in this workspace are held by
content-free assertions in the rejection tests, and a survivor is a reason to
write a test, never a reason to add an exclusion.

### Reading survivors

`scripts/mutants_gate.py` prints a per-crate table and then every surviving
mutant with its file, line and the mutation applied. `unviable` mutants never
compiled and `timeout` mutants never finished, so neither counts as caught or
as missed; the caught percentage is `caught / (caught + missed)`. A crate for
which that denominator is zero — every mutant unviable or timed out, which
means the run measured nothing — is reported as `FAIL (no viable mutants)` and
fails the gate, rather than dividing into a vacuous 100 %.

A survivor is one of three things, and the difference matters:

1. **A gap.** The mutation changes an observable rejection, limit, ordering or
   code-mapping decision and no test looks. This is the common case and the
   only correct response is a test. The seeding run's gaps became the
   boundary and rendering tests listed in the [test layout](#test-layout)
   table above.
2. **An equivalent mutant.** The mutated program behaves identically, so no
   test can exist. `1 << 0` and `1 >> 0` are the same flag constant; an `|`
   of disjoint single bits is the same as an `^`; deleting a match arm that
   falls through to a catch-all producing the same value changes nothing.
3. **Defence in depth.** Two independent guards refuse the same condition, so
   removing either leaves the other to refuse it, with the same stable code.
   Testing one in isolation would mean reaching past the other.

The survivors left after the seeding pass, and why each is left:

| Where | Why it survives |
| --- | --- |
| `archive/central.rs` `1 << 0` → `1 >> 0`, and `\|` → `^` between the encryption flag bits | Equivalent. Both spell the same constant: shifting by zero is identity, and the three flags are disjoint single bits, so their union and their symmetric difference are the same value. |
| `archive/central.rs` `compressed_size > image_bytes` → `>=` or `==` | Defence in depth. This is an early bound on a declared size; an image whose declared size reaches or passes its own length is refused as `archive.truncated.entry_data` by the local-header verification either way, with the same code and the same entry index. |
| `archive/local.rs` `data_end > bytes.len()` → `>=` or `==` | The same early bound on the other side. Entry data is always followed by the central directory and the end record, so `data_end == bytes.len()` cannot occur in an image that got this far. |
| `archive/inflate.rs` `bytes_written > 0` → `>=` | Equivalent. The guard skips an accounting call for an empty chunk; accounting an empty chunk adds zero bytes, updates the CRC with nothing and compares the same totals. |
| `archive/inflate.rs` the `bytes_consumed > 0 \|\| bytes_written > 0` progress guard, in all its forms | This is the anti-stall guard: it refuses a decoder that returns `Ok` while consuming and producing nothing, which would otherwise loop forever. `miniz_oxide` does not enter that state for any input the fixtures can build, so the branch is unreachable from the public API and is kept as a bound on a future decoder change. Mutating it away produces a hang, which the run reports as a timeout rather than a survivor when it does bite. |
| `metadata/scanner.rs` `Scanner::step` → `Ok(None)` (timeout) | Not a survivor: the mutated scanner never advances, so the run hits `minimum_test_timeout` and is reported as a timeout. Timeouts count on neither side of the gate. |
| `cli/render/human.rs` `rest.len() - valid` → `+` (timeout) | Equivalent and slow. The expression is the fallback length for an incomplete final UTF-8 sequence; the result is immediately clamped with `.min(rest.len())`, so both spellings produce the same end offset. |
| `cli/exit.rs` `max_archive_bytes + 1` → `-` or `*` | A gap the fixture cost does not justify. Distinguishing the three requires an input of exactly 64 MiB — the whole point of the cap being one byte past the ceiling — and every test in this suite builds its package in memory. Left for a fixture strategy that can stream one. |
| `cli/commands/mod.rs` deleting the `StructureSummary::Unresolved` arm, and `cli/commands/inspect.rs` deleting the `ReferenceResolution::Missing` arm | Equivalent. Both enums are `#[non_exhaustive]`, so each match already has a catch-all, and in both cases the catch-all produces exactly the value the deleted arm produced. The arms are there so the intended reading is written down, not because the fallback differs. |
| `cli/exit.rs` deleting the `ProfileError::Archive` arm of the `Failure` conversion | Not reached by any command. Every command reads the archive inventory before the structural checks, so an archive-layer refusal becomes a `Failure` directly and this arm only matters for an archive error raised *during* checking — which needs an entry that decodes past a limit only when a check re-reads it. `diagnostics.rs` pins the numbers a limit failure carries through `validate-structure`; the wrapped path itself is left until a fixture can produce it. |
| `cli/render/human.rs` deleting the `(Some(name), None)` arm of `metadata_entry` | The state is unreachable through the commands: a located metadata entry always carries its central-directory index, so a name without one cannot be produced. The arm is defensive. |
| `cli/extract/preflight.rs` `reparse_point` and its `FILE_ATTRIBUTE_REPARSE_POINT` mask | Windows-only code, not compiled on the platform the mutation run was measured on, so no mutant of it could be executed there at all. The rule is covered on the platform that has it: the `cfg(windows)` `mod junctions` and `mod symbolic_links` tests put a directory junction and a real Windows symbolic link in each link position, and the Windows CI lane runs them. |
| `cli/extract/preflight.rs::marker` → `Ok(())`, and its `NotFound` guard | Defence in depth. Preflight refuses an existing marker before anything is written, and the writer's `create_new` refuses it again if one appeared in between; removing preflight's copy leaves the writer's to produce `output.partial_marker_present`, which is why the writer's arm *is* caught and this one is not. |
| `cli/extract/preflight.rs`'s remaining `ErrorKind::NotFound` guards, and `cli/extract/writer.rs`'s `AlreadyExists` guard in `directories` | Treating an unexpected I/O error as the expected one lands on the same stable code by another route: the run continues and the next operation on the same path fails as `output.io`. Producing a non-`NotFound` `lstat` failure at exactly the right path, portably, is what a test here would have to do. |
| `cli/extract/preflight.rs::marker_named_directory` `&&` → `\|\|` and `==` → `!=` | Equivalent in effect. Widening the clash test only reaches the second half of the function, which then finds no item whose first component is the marker name and returns `None`, exactly as before. |
| `cli/extract/cleanup.rs::Ledger::forget` | Unobservable. `forget` is called once, for the marker, on the success path — after which nothing else runs and `undo` is never called. Its effect is only visible to a cleanup pass that cannot happen. |

Nothing in that list may be answered by raising a floor. If a survivor is a
gap, it gets a test; if it is not, it gets a row here saying why.

### Updating the floors

`scripts/mutants-floors.txt` holds one crate per line — its name and its
floor separated by a tab — where the floor is the caught percentage that
crate must not fall below. The floors were seeded from
the first full run, rounded down two points, so ordinary noise — a mutant that
times out on a loaded runner instead of being caught — does not fail the lane.

Raise a floor when a change genuinely raises the score, in the same pull
request:

```sh
cargo mutants --workspace --jobs 2 --output tmp
python3 scripts/mutants_gate.py --dir tmp/mutants.out
```

Read the caught percentage out of the table, subtract two points, round down,
and commit the new value. Never raise a floor to make a survivor disappear,
and never lower one to make a red lane green: a drop means either a test got
weaker or a new branch arrived untested, and both are the finding the lane
exists to produce. CI never writes this file.

## Sweeps

Two exhaustive sweeps sit under the [fuzz targets](#fuzzing) and cover what a
short bounded fuzzing run cannot: they are exhaustive rather than random,
cheap, deterministic, and they run in the normal test suite on every platform.

**Truncation.** For each representative input, every prefix — every length
from zero up to one byte short of the whole — is fed to the parser, and each
must be rejected without panicking. The archive sweep covers a marker-only
archive, an empty archive, a two-entry archive with a data descriptor, and an
archive with a comment. The metadata sweep does the same for a valid
document, and separately truncates every *refused* document as well. A third
sweep truncates a whole KRX-shaped archive through `profile::check`.

What that guarantees: no partial structure can be accepted as a complete one,
and no length-driven index can run past the end of the slice. Combined with
the exact-coverage rule, it also means a valid archive cannot be made valid
again by cutting it short.

**Single-byte mutation.** Every byte position of a valid image is replaced,
in turn, with each of `0x00`, `0xff` and `0x50` (the `P` of a ZIP
signature), and the parser must return — accept or reject — without
panicking. `0x50` is included deliberately: it manufactures stray record
signatures inside entry data, which is the shape that confuses a reader that
scans for signatures rather than following the declared structure.

What that guarantees: no reachable arithmetic overflow, slice index or
`unwrap` on any input one byte away from a valid one. It does not guarantee
anything about inputs two bytes away; that is what the [fuzz
targets](#fuzzing) are for.

## Property-based tests

The sweeps above are exhaustive over one dimension of one fixed input. The
property tests are the other half: they generate the *input* and assert the
contract over whatever comes out. They live in
`crates/openkrx-core/tests/property_round_trip.rs` and
`crates/openkrx-core/tests/property_repack.rs`, with every strategy in the
`crates/openkrx-core/tests/support/strategies.rs` both include, and they use
[`proptest`](https://crates.io/crates/proptest) — a dev-dependency of
`openkrx-core` alone, with default features off, so no shipped binary and no
other crate carries it.

### What is generated

A `PackageSpec` the writer accepts under `Limits::DEFAULT`, so that a refusal
is a finding rather than a generator accident:

| Part | Range |
| --- | --- |
| `Metadata` | Built by serialising a drafted `KER_META_V0_9` document and parsing it back, because `Metadata` and its parts are `#[non_exhaustive]` and a parse is the only honest way to obtain one. Both M4 enumerations in full; every optional header element and every `ERKEZTETES`/`BONTASOK`/`TERTIVEVENY` marker block present or absent independently; `TESZT` present or absent (M11); one `EXPEDIALAS` block, with or without an unqualified `KEZELESI_UTASITASOK` (M8). |
| Text | Trimmed, and drawn from an alphabet of Latin, Hungarian and punctuation characters plus `&`, `<` and `>`, so escaping is exercised rather than avoided. Anything untrimmed or outside XML 1.0 is what the writer refuses, and is generated only by the tests that assert those refusals. |
| Attachments | Zero to eight per package. File names come from a component alphabet holding no separator, colon, control or Windows-reserved character, never start or end with a space and never end with a dot, include precomposed (NFC) Hungarian letters, and are filtered of Windows reserved device names — the union of the archive layer's name rules and the extraction planner's component rules. `MELLEKLET_LEIRASA`, `MENNYISEG` and `MENNYISEGI_EGYSEG` present or absent. |
| Attachment bytes | Nothing, small incompressible noise, a run of zero bytes up to and including `Limits::RATIO_GRACE_BYTES`, and incompressible bytes just past that grace. The compressible arm stops *at* the grace deliberately: one byte more and the writer would apply the reader's compression-ratio ceiling to it and refuse the package. |
| `FixedTimestamp` | The MS-DOS epoch, or any date and time inside the representable 1980–2107 range. |

The repacking properties draw from the same generators, with two additions:

| Part | Range |
| --- | --- |
| A request repacking accepts | The same `PackageSpec`, with the four things repacking refuses in an *input* held off — the `ERKEZTETES`, `BONTASOK` and `TERTIVEVENY` marker blocks and the unqualified `KEZELESI_UTASITASOK` element, whose presence is all the reader retains — and with the one `EXPEDIALAS` block always present, so that the container-less empty dispatch is generated at all. Zero to four attachments rather than eight, because a repacking case writes a package, reads it and writes it again. The two properties that need one container form or the other draw a document built with it rather than filtering for it: one drawn document in ten has no attachment and no container, and a filter for that exhausts proptest's local reject budget on a long run. |
| `Edits` | Any subset of the ten `FEJRESZ` fields, each optional element kept, set or cleared; zero to three additions with the file names and bytes the writer properties use; and each of the package's own attachment numbers independently left alone, removed or replaced — which makes “no target named twice” true by construction rather than by a filter. The invalid arm draws a number the package does not carry, zero included, or one number named twice. |

### The writer and reader properties

1. **Round trip.** `package` → `archive::inventory` → `metadata::parse` of the
   metadata entry → `profile::check` reports no `Fail`, and the summary is
   `Unresolved` — A19 leaves the `KRX/OCD/` marker prefix open and M13 the unit
   of `MERET`, and no package the writer produces resolves either. Every
   attachment's `entry_bytes` equals the input byte for byte. The parsed
   document equals the **documented normalised form** of the input: the header,
   the three marker blocks and the `KEZELESI_UTASITASOK` flag come back
   unchanged, and the single dispatch's `MELLEKLET` list and
   `MELLEKLETEK_SZAMA` are the ones derived from the attachments actually
   written — the 1-based number, the file name, the `KRX/OCD/Payload/ID-<n>`
   location and `MERET` in kilobytes rounded up (M6). That form is also a fixed
   point: handing it back with the same attachments writes the same bytes.
2. **Determinism.** One request, written twice, is byte-identical output.
3. **Mutation.** One byte of a written package is changed, and the reader
   either refuses the image or stays inside every ceiling it was given. The
   property is deliberately not "a mutation is always detected" — a byte in an
   unused header field need not be — but that nothing panics and no limit is
   exceeded. Nothing in the test is unwrapped, so a panic can only come from
   the crate under test.
4. **Limits.** One documented ceiling — entry count, name bytes, entry size,
   total size, image size or compression ratio — is tightened to just below
   what the request needs, derived from the package the request actually
   writes rather than guessed, and the refusal must carry that ceiling's
   `create.over_limit.*` code.
5. **Planning.** `extract::plan` accepts the inventory of every written
   package, plans exactly one item per entry, and each item's components join
   back to the entry name.

### The repacking properties

Repacking composes the reader, the parser and the writer, so its corner cases
are the products of three surfaces rather than one. The six properties in
`property_repack.rs` are:

1. **Identity.** `plan` with an empty `Edits`, then `apply`, is byte-identical
   to the input for every package the writer produces that repacking accepts —
   both `MELLEKLETEK` container forms included, now that the reader retains
   which one a dispatch carried. This is the rule every `repack.unsupported.*`
   refusal exists to keep true. The plan reports nothing preserved but the
   attachments, nothing changed, removed or added, and no header field.
2. **Add and remove.** Adding one to three attachments and then removing
   exactly the numbers the plan gave them restores the input byte for byte, for
   every package whose document already lists an attachment or already carries
   the `MELLEKLETEK` container. The numbers removed are the plan's own
   `added()`, not numbers the test computed: an addition takes the number the
   *result* gives it.
3. **The one asymmetry.** For the one remaining shape — an empty dispatch
   carrying no `MELLEKLETEK` container — the same round trip differs by exactly
   that empty container and by nothing else, and is an exact identity from then
   on. See [the container asymmetry](#the-container-asymmetry) below for why,
   and for what the property pins.
4. **Preservation.** After any valid edit list, every attachment no edit named
   reads back byte-identical to the input's bytes *at the number the edit
   spared*, not at its output position: removing an attachment renumbers
   everything after it. The replaced attachments carry the supplied bytes, the
   additions land after them in order, the plan's preserved, changed, removed,
   added and header-field lists are exactly the edits it was given, and the
   result fails no structural check and is `Unresolved` — no more and no less
   than a package the writer produced.
5. **Refusal.** An edit naming an attachment number the package does not carry,
   zero included, is refused with `repack.invalid.no_such_attachment`, and one
   naming a number twice with `repack.invalid.duplicate_target`. Nothing in the
   test is unwrapped, so a panic can only come from the crate under test.
6. **Determinism.** Planning the same package twice decides the same thing,
   whether that is the same plan or the same refusal with the same entry index
   and attachment number. The request comes from the unrestricted strategy, so
   most cases carry a block the reader records only the presence of and are
   refused with `repack.unsupported.opaque_block`: a refusal a caller cannot
   reproduce is one they cannot act on.

### The container asymmetry

Adding an attachment to a dispatch that carried no `MELLEKLETEK` container and
then removing it again does **not** restore the original bytes: the result
carries an empty container the input never had. That is documented behaviour
rather than a defect, and it is inherent to the composition. Rule M7 makes
`MELLEKLETEK` and `MELLEKLETEK_SZAMA` separate elements, so an empty container
and no container at all are different documents; the writer must emit the
container to hold the added reference, so the intermediate package genuinely
carries one; the reader retains that fact rather than normalising it away; and
the later removal cannot know the original had none, because nothing in the
package it is given says so. Property 3 pins where the difference stops: the
marker and every attachment stay byte-identical, the document differs by
exactly one empty `MELLEKLETEK` element, the parsed documents agree on
everything but `attachments_present`, the result still fails no structural
check, and a second add-and-remove over it is an exact identity — the shift
happens once and never drifts further.

### Case counts and regressions

Each property runs **64 cases**, which keeps the whole file well under a
second locally. `PROPTEST_CASES` overrides that for a deliberate long run:

```sh
PROPTEST_CASES=4096 cargo test -p openkrx-core --locked --test property_round_trip
PROPTEST_CASES=4096 cargo test -p openkrx-core --locked --test property_repack
```

A failing case is shrunk and its seed persisted under
`crates/openkrx-core/proptest-regressions/`, one file per property. Those files
are **committed**: the seeds are synthetic by construction — every byte they
reproduce comes from the strategies above, and no real package can reach them —
so a case found once is re-run first by everyone thereafter. The directory is
deliberately not ignored by Git, and is empty today because every property
passes.

Property tests are not a mutation-testing target: `cargo mutants` mutates
`crates/*/src/**` only, and everything here lives under `tests/`. See
[mutation testing](#mutation-testing).

## Fixture policy

The policy is stated in
[tests/fixtures/README.md](../tests/fixtures/README.md), which is also the
inventory of the only committed packages.

Every archive and every document a *Rust* test reads is generated at run time
by `crates/openkrx-core/tests/support/`, an independent original written for
this repository that can emit contradictory headers on purpose. Generating
beats committing: the property under test is visible in the test that builds
the input, and no opaque binary has to be trusted or re-derived.

The five packages under `tests/fixtures/golden/` are the documented
exception, and they are generated too — just ahead of time. The golden
contract compares process output byte for byte, so the input has to be the
same bytes on every machine and in every run, which a builder called from
inside a test cannot guarantee across a refactor of that builder.
`crates/openkrx-core/examples/golden_fixtures.rs` writes them from the same
synthetic writer, from fixed values with no clock and no randomness:

```sh
cargo run -p openkrx-core --features synthetic-writer \
    --example golden_fixtures -- tests/fixtures/golden
```

Re-running it must rewrite byte-identical files, and
`python3 scripts/golden.py verify-fixtures` asserts that in CI, so a
hand-edited package cannot quietly become the contract.

Any further binary fixture may be added only if a property genuinely cannot
be generated. It must then be recorded in the fixture inventory with its
provenance, the property under test and the expected result, and it must be
an independently authored synthetic original — never a real submission, and
never a redacted or modified derivative of one. Third-party example packages
and schemas need documented redistribution permission before they may be
copied here; public accessibility is not permission.

`metadata_evidence.rs` is the one place that touches official material, and
it touches only its shape: an XML declaration, the target namespace bound to
an `ns2` prefix, and a reference split across `ELHELYEZKEDES` and
`FAJL_NEV`. No identifier, description, timestamp, barcode or note is copied
from any source, and none may be added.

## Fuzzing

Both readers, the structural checks, the extraction planner and the writer
have a `cargo-fuzz` target. The package lives in
[fuzz/](../fuzz/README.md) and is **not** a member of the root workspace: it
needs a nightly toolchain and libFuzzer, and keeping it separate means
`cargo deny`, `cargo llvm-cov --workspace` and the MSRV check never see its
dependencies. `cargo machete` does scan the directory, and needs no exclusion:
`libfuzzer-sys` is reached through a normal `use`, so it is seen as used.

| Target | What it calls |
| --- | --- |
| `inventory` | `archive::inventory(data, &Limits::DEFAULT)`, then `entry_bytes` for every accepted entry, which is the only path that inflates data |
| `xml_metadata` | `metadata::parse(data, &MetadataLimits::DEFAULT)` |
| `structure` | `archive::inventory`, then `profile::check(&inventory, &MetadataLimits::DEFAULT)` over what it accepted |
| `extract_plan` | `archive::inventory`, then `extract::plan(&inventory, &ExtractLimits::DEFAULT)` |
| `create_round_trip` | `create::package(&spec, &Limits::DEFAULT)` on a `PackageSpec` built from the fuzzer's bytes with `arbitrary`, then `create::verify_round_trip` |

The first three assert nothing about the result. The property under test is
that the call **returns** — accept or refuse — on any byte string, which is the
same property the sweeps hold one byte at a time.

The last two additionally assert one invariant each. They are invariants of
this crate, not conformance claims about the KRX format, and each is worded so
that only a defect can break it:

- **`extract_plan`: a produced plan is joinable.** Every component of every
  planned item is non-empty, is neither `.` nor `..`, and holds no `/` or
  backslash, and the first component is non-empty so the destination is never
  absolute. That is the planner's documented promise; a caller joins the
  components against its own destination and nothing else stands between the
  archive's names and the filesystem.
- **`create_round_trip`: what the writer wrote, the reader accepts.** When
  `create::package` returns bytes, `create::verify_round_trip` must produce a
  report with no `CheckOutcome::Fail`, and each attachment must read back
  byte-identically through `entry_bytes`. When it refuses, the code must start
  with `create.` and must be one `docs/codes.md` catalogues — the target reads
  the catalogue with `include_str!` and scans it, so a new code that is never
  documented fails the lane rather than reaching a caller undocumented.

The request the round-trip target builds is bounded on purpose: header strings
are printable, whitespace-trimmed and at most 32 characters, there are 0 to 4
attachments of at most 4 KiB each, and the timestamp is taken as the two MS-DOS
fields, which are in range by construction. File names are *not* trimmed or
otherwise cleaned, so the `create.unsafe_name.*` classes stay reachable.

### Prerequisites

```sh
rustup toolchain install nightly
cargo install cargo-fuzz
```

libFuzzer ships with the nightly toolchain, so nothing else is needed on
Linux or macOS. There is no Windows lane.

### Running a target

```sh
cargo +nightly fuzz build --fuzz-dir fuzz
cargo +nightly fuzz run --fuzz-dir fuzz inventory -- -max_total_time=30
cargo +nightly fuzz run --fuzz-dir fuzz xml_metadata -- -max_total_time=30
cargo +nightly fuzz run --fuzz-dir fuzz structure -- -max_total_time=30
cargo +nightly fuzz run --fuzz-dir fuzz extract_plan -- -max_total_time=30
cargo +nightly fuzz run --fuzz-dir fuzz create_round_trip -- -max_total_time=30
```

Without `-max_total_time` a run continues until it is interrupted. The corpus
it accumulates is written to `fuzz/corpus/<target>/` and is ignored by Git;
delete it to start from nothing. Add `-rss_limit_mb=2048` on a memory-tight
machine, and run one target at a time.

### The seed corpus

libFuzzer starts from whatever `fuzz/corpus/<target>/` already holds, and a
fresh checkout holds nothing. Seed it first:

```sh
python3 fuzz/seed.py            # build or refresh every target's corpus
python3 fuzz/seed.py --verify   # assert it exists; print the counts only
```

[fuzz/seed.py](../fuzz/seed.py) is Python 3 with the standard library and
nothing else. It derives its seeds from the committed golden fixtures under
`tests/fixtures/golden/`, which are synthetic by the [fixture
policy](#fixture-policy) and are the only files it reads:

| Target | Seeds |
| --- | --- |
| `inventory`, `structure`, `extract_plan` | each `.krx` fixture as it stands, the malformed one included |
| `xml_metadata` | the `KULDEMENY_META.xml` member extracted from each fixture that is a readable container |
| `create_round_trip` | that member's bytes concatenated with the payload members', one blob per fixture |

The manifest JSON files are not seeds: no target parses a manifest.
`create_round_trip` reads its input through `arbitrary` rather than as a
package, so any byte string is a valid seed for it — real XML text and real
file content simply give the mutator better material to draw header strings
and attachment bodies from.

Nothing produced is committed. `fuzz/corpus/` is ignored by
[fuzz/.gitignore](../fuzz/.gitignore), and seeding is a step before a run, not
a state of the repository. The script writes its seeds by name and overwrites
only its own, so seeding a corpus libFuzzer has already grown is safe and
idempotent.

Seeding changes what a bounded run means. A 30-second `inventory` run from an
empty corpus reached 144 edges and 201 features after 20,504,197 executions,
on a corpus of 53 inputs. The seeded run *began* at 514 edges and 687
features — more than the unseeded run ever found — and ended at 798 edges and
2,349 features after 2,382,542 executions. The execution count falls by an
order of magnitude because each input is now a real package of 1 to 31 KB
rather than a handful of bytes, which is the trade being made: fewer, deeper
executions.

### Running a campaign locally

A campaign is the same targets on a budget worth the name, from a seeded
corpus that survives between runs:

```sh
python3 fuzz/seed.py
for target in inventory xml_metadata structure extract_plan create_round_trip; do
  cargo +nightly fuzz run --fuzz-dir fuzz --target x86_64-unknown-linux-gnu \
    "$target" -- -max_total_time=1200 -rss_limit_mb=2048
done
```

Run one target at a time; each one uses every core it is given. The corpus
grows across runs and is worth keeping — delete `fuzz/corpus/` only to
measure a cold start. What the accumulated corpus reaches is measurable:

```sh
rustup component add llvm-tools-preview --toolchain nightly
cargo +nightly fuzz coverage --fuzz-dir fuzz \
  --target x86_64-unknown-linux-gnu inventory
```

That replays the corpus through an instrumented build and merges the profiles
into `fuzz/coverage/inventory/coverage.profdata`. It renders no report, so
`llvm-cov report` from the toolchain's own `llvm-tools-preview` does that,
against the instrumented binary cargo-fuzz leaves under
`target/<triple>/coverage/` — which is where the weekly lane below reads it
from too.

### Reproducing an artifact

A crash writes its input to `fuzz/artifacts/<target>/`. Replay and minimise it:

```sh
cargo +nightly fuzz run  --fuzz-dir fuzz inventory fuzz/artifacts/inventory/crash-<id>
cargo +nightly fuzz tmin --fuzz-dir fuzz inventory fuzz/artifacts/inventory/crash-<id>
cargo +nightly fuzz fmt  --fuzz-dir fuzz inventory fuzz/artifacts/inventory/crash-<id>
```

`tmin` shrinks the input while it still reproduces; `fmt` prints it as the
Rust literal to paste into a test. A crash is then handled by the rule in
[fuzz/regressions/README.md](../fuzz/regressions/README.md): minimise, express
it as a named generated construction in the matching `*_rejects_*.rs` file,
and only then fix the reader. The minimised blob is retained under
`fuzz/regressions/<target>/` only when it cannot be expressed as a
construction, and only because it is fuzzer-generated and therefore synthetic.

### Adding a target

Add `fuzz/fuzz_targets/<name>.rs` holding one `fuzz_target!` call and nothing
else, add the matching `[[bin]]` block to `fuzz/Cargo.toml` with
`test = false`, `doc = false` and `bench = false`, create
`fuzz/regressions/<name>/.gitkeep`, extend the table above and add the target
to both lanes below — the list and the replay loop in
`.github/workflows/ci.yml`, and in `.github/workflows/fuzz.yml` the run loop,
the minimisation loop, the size-recording loop and its own pair of corpus
`restore`/`save` steps, which are per target by design — and to `fuzz/seed.py`
if there is fixture material it can start from. Keep the
harness body to the single call unless there is an invariant to hold: a harness
that asserts a *result* turns a behaviour change into a fuzzing failure, which
is not what this lane is for, while an invariant — something only a defect can
break, such as the two above — belongs in the harness and is documented with
it.

### The CI lane

The `Fuzz (build only)` job in `.github/workflows/ci.yml` runs on
`ubuntu-latest` for every push and pull request. It installs the nightly
toolchain and a pinned `cargo-fuzz`, builds the five targets, seeds the corpus
with `python3 fuzz/seed.py`, then runs each target for **30 seconds** with
`-rss_limit_mb=2048`, a 150-second total fuzzing budget. Between the build and
the seeding it replays every retained regression, described
[below](#replaying-the-retained-regressions).
Crash artifacts are uploaded when the job fails, for one day: the retained
regressions this lane replays are committed under `fuzz/regressions/`, so the
upload is a run-scoped convenience, not the durable copy. The job names
`--target x86_64-unknown-linux-gnu` explicitly, through the
`FUZZ_TARGET_TRIPLE` env variable: cargo-fuzz otherwise defaults to the triple
of its own binary, and the pre-built one is a musl build, which
AddressSanitizer cannot link against a static libc.

The lane also enforces `fuzz/Cargo.lock`. `cargo fuzz` accepts none of cargo's
manifest flags — `--locked` reaches neither `fuzz build` nor `fuzz run`, both
of which reject it as an unexpected argument — so the job resolves the same
dependency graph first with

```sh
cargo +nightly metadata --manifest-path fuzz/Cargo.toml --locked --format-version 1
```

which fails when the lockfile is absent or stale. The build that follows then
reuses that lockfile unchanged, which is what `--locked` would have bought.

The budget is deliberate. Thirty seconds per target proves the harness still
links and executes and catches a shallow regression — the class of bug a
refactor introduces — without adding minutes to every CI run. It is not
a campaign, and the job passing is not evidence that a reader is fuzz-clean,
only that it survived a short bounded run from a seeded corpus. The campaign
is the [weekly lane](#the-weekly-campaign-lane).

### Replaying the retained regressions

The `Replay the retained regressions` step in the same job runs every file
under `fuzz/regressions/<target>/` through the built target and fails the job
on a crash:

```sh
cargo +nightly fuzz run --fuzz-dir fuzz --target x86_64-unknown-linux-gnu \
  inventory fuzz/regressions/inventory -- -runs=0 -rss_limit_mb=2048
```

The directory is passed as the corpus and `-runs=0` adds no mutation, so
libFuzzer executes each file in it once and exits. That is what makes a
retained crash a permanent gate rather than a blob nobody re-checks: a fixed
crash that comes back fails the next push. It runs before the seeding, on
every push and pull request, and carries a five-minute step timeout — replaying
a handful of minimised inputs costs about what the process starts cost, so a
step that runs long is a defect in itself.

A target whose directory holds nothing but its `.gitkeep` is skipped and the
step says so. **Every directory is in that state today**: no crash has ever
been found, so the step reports five skips and passes. It needs no edit when
the first blob lands — the rule for when one may, in
[fuzz/regressions/README.md](../fuzz/regressions/README.md), is unchanged, and
a named generated construction in a `*_rejects_*.rs` file remains the preferred
form of the evidence. This step is the backstop for the inputs that cannot be
written that way.

### The weekly campaign lane

`.github/workflows/fuzz.yml` is the long run, on the model of
[mutants.yml](#mutation-testing): **weekly on a schedule**, plus
`workflow_dispatch` with a `minutes_per_target` input that defaults to **20**
and is refused outside 1–240, so a mistyped budget fails in the first step
rather than by hitting the job's 360-minute timeout hours later.
It is deliberately **not** a required check and is not listed in
`.factory.yaml`. A crash it finds is a defect to triage under
[fuzz/regressions/README.md](../fuzz/regressions/README.md), not a merge
block on whatever happened to be in flight.

The job is the CI lane's steps on a larger budget: the same pinned actions,
the same nightly toolchain and pinned `cargo-fuzz`, the same `fuzz/Cargo.lock`
gate, `python3 fuzz/seed.py`, then each of the five targets for the budget
with `-rss_limit_mb=2048`. It then runs `cargo +nightly fuzz coverage` for
`inventory` — the entry point the other package targets all go through —
renders the report with `llvm-cov` and puts it in the job summary. That step
is an ordinary one whose failure fails the job: `llvm-tools-preview` installs
cleanly as a component of the nightly toolchain, which was verified before
the step was written, so a failure there is a real one.

#### The cumulative corpus

The campaign is cumulative: week n+1 starts from everything week n reached,
not from the fixtures alone. Each target's corpus is a separate
[`actions/cache`](https://github.com/actions/cache) entry, and the campaign
walks it through four states:

1. **Restore.** `fuzz/corpus/<target>/` is restored from the previous
   campaign's entry. The key is
   `fuzz-corpus-<generation>-<target>-<ISO week>-<run id>`; the run id makes
   every campaign write a key of its own, because `actions/cache/save` refuses
   to overwrite an existing key and a second run in the same week would
   otherwise throw its work away. Reading is therefore done by the restore
   keys — the week prefix first, then the bare target prefix — and a prefix
   match returns the most recently created entry, so a restore always lands on
   the latest campaign. A miss is not an error: the directory stays empty and
   the seeding below fills it.
2. **Seed on top.** `python3 fuzz/seed.py` writes its seeds by name and
   overwrites only its own, so it refreshes the fixture-derived inputs without
   disturbing anything libFuzzer added in an earlier week.
3. **Fuzz.** Each target for the budget, exactly as before.
4. **Minimise and save.** `cargo +nightly fuzz cmin` merges each corpus into a
   fresh directory, keeping only the inputs that contribute a feature no
   earlier input already covers, and replaces the corpus with it. Coverage is
   preserved by construction while the input count and the byte total fall,
   which is what stops a corpus carried forward every week from growing without
   bound. That minimised directory is what is saved as the new cache entry.
   Saving happens before the coverage pass, so a failure there cannot cost the
   campaign hours of accumulated inputs, and it is skipped on the pull-request
   rehearsal, whose two minutes per target are not what the next campaign
   should start from.

The job summary reports **inputs and bytes per target, before and after**:
"before" is measured after the restore and the seeding, so it is what the
campaign actually started from, and "after" once the minimisation is done.
Read together across weeks, those two columns are the evidence that the corpus
is compounding rather than churning.

**Downloading it.** The `fuzz-campaign` workflow artifact of any campaign run
carries `fuzz/corpus` alongside `fuzz/artifacts` and `fuzz/coverage`, for 14
days — a documented exception to the one-day retention baseline, recorded
under [artifact retention](releasing.md#artifact-retention). Take it from the
run page, or:

```sh
gh run download --repo watt-mind/openKRX <run-id> --name fuzz-campaign
```

and unpack `corpus/<target>/` over the local `fuzz/corpus/<target>/`. A cache
entry itself is readable only by a workflow run, so the artifact is the
maintainer-facing copy — and on a crash it is the only copy, because the save
steps do not run when a target fails.

**Resetting it.** Bump `CORPUS_CACHE_VERSION` in
`.github/workflows/fuzz.yml` — `v1` to `v2`. Every key is prefixed with it, so
every restore misses and the next campaign starts from the seeds alone. That
is what a maintainer wants after a change to what an input *means*: a limit, a
harness, or a target's entry point, when the accumulated inputs are exploring a
shape the reader no longer has. Nothing is deleted; the old entries stop being
addressed and expire on GitHub's own schedule.

`fuzz/artifacts`, `fuzz/corpus` and `fuzz/coverage` are uploaded as a
workflow artifact with a 14-day retention, `if: always()` — on a crash the
artifacts directory holds the crashing input and the corpus is what a
maintainer replays it against, and neither is committed.

A `workflow_dispatch` workflow is registered only from the default branch, so
this lane cannot be dispatched from the branch that adds it. It therefore also
carries a `pull_request` trigger filtered to `.github/workflows/fuzz.yml` and
`fuzz/seed.py`, which runs the identical job at a 2-minute budget — the same
rehearsal device [release.yml](../.github/workflows/release.yml) uses, and for
the same reason. An ordinary pull request touches neither file and is served
by the 30-second lane above.

Outstanding:

- **The persisted corpus is a cache, not a repository.** GitHub evicts a cache
  entry that has not been read for seven days and enforces a repository-wide
  size limit, so a long enough gap between campaigns — a paused schedule, a
  quota pushed over by another lane — silently returns the next run to the
  seeds. The weekly cron reads the entry often enough that this should not
  happen, and the before/after columns in the job summary are where it would
  show; nothing alerts on it.
- **Coverage is measured for one target only**, `inventory`, and is reported
  rather than gated: no floor exists that a drop would fail.
- **No campaign has run against a real package.** Every seed is synthetic by
  the [fixture policy](#fixture-policy), so the shapes the corpus explores are
  the shapes the fixtures already have.

Until those are addressed, the sweeps above remain the load-bearing
compensating control, and the residual risk stays recorded in
[roadmap.md](roadmap.md#residual-risks-in-the-current-state).

## Benchmarks

Every value in `Limits::DEFAULT` is a ceiling, so a maximal-but-valid package
is something a caller can be handed at any time: 64 MiB of image, 256 entries,
32 MiB in one entry, 128 MiB decoded in total. The benchmarks measure the cost
of that package, and the scaling guard below turns "the cost grew the wrong
shape" into a failing test.

```sh
cargo bench -p openkrx-core          # the whole set
cargo bench -p openkrx-core --no-run # compile only, locally
cargo bench -p openkrx-core -- inventory   # one group
```

No workflow runs `cargo bench`: a timing number from a shared runner is not
something to gate a merge on. What CI does hold is that the targets still
compile — the clippy gate lints `--all-targets` and the MSRV job runs `cargo
check --all-targets`, and both of those include `benches/`. A benchmark that
stops building therefore fails the pull request; one that gets slower does not.

The harness is [`criterion`](https://crates.io/crates/criterion), a
dev-dependency of `openkrx-core` alone, with `default-features = false` and
only `cargo_bench_support` enabled — without that one feature criterion
refuses to run under `cargo bench` at all, and the two it replaces (`rayon`
and `plotters`) are a thread pool and a plotting stack for reports nothing
here reads. Its declared minimum Rust is 1.86, inside the workspace's 1.88.
There is no runtime dependency: no binary contains any of this.

**Every package a benchmark reads is generated in its own setup**, by the
test-only writer behind the `synthetic-writer` feature or by
`create::package`, and nothing is committed. A benchmark corpus is not an
exception to the [fixture policy](#fixture-policy): `docs/profile.md` records
that no redistribution licence exists for the primary sources.

| Target | Group | What it measures |
| --- | --- | --- |
| `benches/reader.rs` | `inventory` | One inventory pass over an image of 1, 16 and 64 MiB, split over eight entries, **stored** and **deflated** separately so the cost of inflate is visible beside the cost of walking the structure; and over a 256-entry package, which is `max_entries`. |
| `benches/reader.rs` | `entry_bytes` | Re-decoding all 256 entries of that package one at a time, over an inventory built in the setup: the work `extract` and `profile::check` do on top of a reading pass already paid for. |
| `benches/reader.rs` | `metadata_parse` | `metadata::parse` on a 10 KiB document and on one whose text nodes come within a few per cent of `max_text_bytes` (1 MiB), spread over 200 attachment descriptions. |
| `benches/planner.rs` | `extract_plan` | `extract::plan` over 32 and 256 entries whose names are long and multi-byte, because the planner folds each component to NFC and then case-folds it to find a collision. |
| `benches/planner.rs` | `profile_check` | `profile::check` over 32 and 254 attachments, each referenced by the document. 254 is `max_entries` less the format marker and the metadata document, so it is a full package. |
| `benches/writer.rs` | `create_package` | `create::package` over a request carrying 1 MiB and 16 MiB of incompressible attachment bytes in 16 attachments: reference derivation, deflate and assembly in one number. |

The deflated and the incompressible cases both use a deterministic xorshift
stream, which deflate cannot shrink. That is deliberate: it keeps the image
the size the label says, keeps the decoded total under
`max_total_decoded_bytes`, and keeps the compression-ratio ceiling out of a
measurement that is about throughput rather than about a refusal.

### The scaling guard

`crates/openkrx-core/tests/scaling_guard.rs` is an ordinary integration test,
so it runs in `cargo test --workspace` and fails CI. It measures the same
operation at a small size and at the ceiling with `Instant`, and asserts a
wall-time ratio:

| Guarded | Sizes | Size ratio | Threshold | Linear would be |
| --- | --- | --- | --- | --- |
| `archive::inventory`, stored entries | 4 MiB, 64 MiB | 16 | < 32 | 16 |
| `extract::plan`, long Unicode names | 32, 256 entries | 8 | < 32 | 8 |
| `profile::check`, referenced attachments | 32, 254 attachments | 8 | < 32 | 8 |

The inventory guard reads **stored** entries so that inflate is excluded and
what is left is the structural walk, the CRC and the exact-coverage check.

The thresholds are loose on purpose. Each sits at about twice linear and far
below quadratic — which would be 256 and 64 — so the guard catches a change of
*shape* and is deliberately blind to a change of constant factor: a shared CI
runner, a debug build and a cold cache all move the constant and none of them
moves the exponent. A guard that flakes gets disabled, and a disabled guard
catches nothing. Two further things keep it honest: every size is measured
twice and the **minimum** is taken, because noise only ever adds time; and the
two count guards — `extract::plan` and `profile::check`, whose single call is
too short to time reliably — repeat the counted operation twenty times inside
one measurement, at both sizes, so the repetition cancels out of the ratio and
only buys a measurement long enough for the clock to resolve. The inventory
guard needs none of that: one pass over 4 MiB already outlasts the clock's
resolution, so it is timed once per measurement.

The test is sized to run in a **debug** build inside CI's budget — about 3.5
seconds on the host below, against a ceiling of 30. Run it alone, with the
observed ratios printed, with:

```sh
cargo test -p openkrx-core --locked --test scaling_guard -- --nocapture
```

Tests are not mutated by `cargo mutants`, so no entry in `.cargo/mutants.toml`
is needed for either the guard or the benchmarks.

### Baseline

**These numbers are host-specific and informational.** They are one local run,
not a budget and not a threshold; nothing fails because a number here moved.
They exist so a reader knows the order of magnitude and can see the shape.

Measured on a 13th Gen Intel Core i9-13900, Linux, rustc 1.98.1, release
profile, median of criterion's estimate:

| Benchmark | Median | Throughput |
| --- | --- | --- |
| `inventory/stored/1MiB` | 1.93 ms | 519 MiB/s |
| `inventory/stored/16MiB` | 31.2 ms | 513 MiB/s |
| `inventory/stored/64MiB` | 131 ms | 489 MiB/s |
| `inventory/deflated/1MiB` | 1.89 ms | 496 MiB/s |
| `inventory/deflated/16MiB` | 38.0 ms | 420 MiB/s |
| `inventory/deflated/64MiB` | 136 ms | 469 MiB/s |
| `inventory/stored/256_entries` | 2.37 ms | 108 Kentry/s |
| `entry_bytes/256_entries` | 1.66 ms | 154 Kentry/s |
| `metadata_parse/10KiB` | 8.62 µs | 1.24 GiB/s |
| `metadata_parse/near_max_text_bytes` | 436 µs | 2.24 GiB/s |
| `extract_plan/unicode_names/32` | 200 µs | 160 Kentry/s |
| `extract_plan/unicode_names/256` | 1.32 ms | 193 Kentry/s |
| `profile_check/attachments/32` | 87.4 µs | 366 Kelem/s |
| `profile_check/attachments/254` | 629 µs | 404 Kelem/s |
| `create_package/1MiB` | 16.8 ms | 59.6 MiB/s |
| `create_package/16MiB` | 351 ms | 45.6 MiB/s |

The whole set takes about three minutes on that host. The guard on the same
host reported ratios of 16.6, 6.4 and 7.3 in a debug build and 15.1, 8.0 and
7.4 in a release one, against a threshold of 32 — every path linear, and none
of the three has a superlinear ceiling to report.

## API compatibility report

The golden output contract above holds the CLI's contract: the bytes a
consumer parses. It says nothing about the other public surface, the Rust API
of `openkrx-core`, where an accidental rename or a removed `pub use` is
invisible to every test in this repository as long as the workspace still
compiles against itself.

[cargo-semver-checks](https://github.com/obi1kenobi/cargo-semver-checks)
answers that question by building the rustdoc JSON of two versions of the same
crate and running SemVer lints across the pair. Its usual baseline is the
crate's last published release; `publish = false` means there is none, so the
baseline here is a git revision — for a pull request, its base commit.

`scripts/api-check.sh [base-rev] [crate]` is the same comparison for a
maintainer, defaulting to `origin/develop` and `openkrx-core`:

```sh
cargo install cargo-semver-checks --locked
git fetch origin develop
bash scripts/api-check.sh origin/develop
```

The script forces `--release-type minor`, and that flag is the whole reason
the run says anything. Both sides of every comparison carry the same
`0.1.0-dev.0` workspace version, and an unchanged version makes
`cargo-semver-checks` assume a major bump — under which every breaking change
is permitted, so all 196 breaking-change lints are skipped and the run reports
nothing whatever the diff did. Declaring the comparison a minor release runs
them, and the question they answer is the useful one: would this change break
a downstream build if the API were already published?

The `API compatibility (informational)` job in `ci.yml` runs the script on
pull requests against `github.event.pull_request.base.sha`. Three properties
are deliberate:

- It is **informational**. The step carries `continue-on-error: true` and the
  job is not in the repository's required checks, so a finding cannot block a
  merge. Nothing is published and no downstream build exists to break, so a
  gate would be a claim the project cannot yet make. The decision to gate
  belongs with the decision to publish; see
  [releasing.md](releasing.md#semver-checks).
- It **skips cleanly**. The comparison runs when `crates/openkrx-core`,
  `Cargo.toml` or `Cargo.lock` differs from the base commit — the manifest and
  the lockfile are in that list because a dependency bump can move a type this
  crate re-exports or derives on, changing the public API without touching a
  line under `crates/openkrx-core`. When none of the three moved, the job says
  so in the summary and installs nothing.
- It reports into the **job summary**. A `PASS`, `BREAK`, `NOT RUN` or
  `SKIPPED` line, plus the tool's own output when there is any, land in
  `$GITHUB_STEP_SUMMARY`, so a reviewer reads the finding on the run page
  instead of opening a log. `NOT RUN` is the case where the toolchain, the
  cache or the install failed before the comparison started: there is no
  result either way, and an informational report that could not run is not a
  finding about the pull request, so the job still ends green.

The job checks out with `fetch-depth: 0`, because `--baseline-rev` resolves a
commit in the local clone and the default single-commit fetch does not contain
the base. It also clears the workflow-level `RUSTFLAGS: -D warnings`: it
compiles the baseline commit too, which is code the pull request did not write
and cannot fix, and a warning there would abort the comparison rather than
report on it.

What the report is not: it compares one crate's public Rust API against one
earlier commit. It says nothing about the CLI contract, which the golden cases
hold, nothing about the JSON envelope, which `schema_version` versions, and
nothing about package-format compatibility, which is the separate open problem
below.

## Compatibility testing later

Once a reader command exists, a compatibility layer becomes possible and
becomes necessary: openKRX is strict on purpose, and strictness has to be
measurable against what real producers emit.

The shape it must take: a request carries a synthetic reproducer, the
expected profile and version, a public source for the rule in question, and
the expected behaviour. An unsupported profile must stay distinguishable
from malformed input and from a resource-limit refusal, which is why the
code categories exist. A finding is resolved either by a documented, tested
exception with its own stable code, or by a recorded decision not to accept
that input — never by relaxing a check to increase compatibility.

A reader/writer round-trip is explicitly not compatibility evidence: it
proves only that our own components agree with each other. Real evidence
needs an official sample archive or an independent implementation, neither
of which was found; see
[conformance.md](conformance.md#known-gaps).

## Private-corpus policy

One private-corpus harness exists, the opt-in corpus check below, and no
second one may be added casually. The rules it obeys are in
[roadmap.md](roadmap.md#private-corpus-policy-for-maintainers) and
[SECURITY.md](../SECURITY.md#data-and-key-policy): opt-in, excluded from
public CI, and reporting aggregate counts and stable error-code buckets
only. No filename, path, metadata field, payload, certificate identity or
hash may enter a report, an issue, a commit or a pull request.

## Private opt-in corpus check

`scripts/private-corpus.py` runs the built executable over a maintainer-local
directory of real `.krx` packages and prints counts alone. It is opt-in: no
test, no `scripts/check.sh` run and no CI job invokes it. It exists so a
maintainer can learn which of the layouts left open by rules A19, A20, M10
and M12 real producers actually emit, without a private byte reaching the
repository.

```sh
cargo build --release --locked -p openkrx-cli
python3 scripts/private-corpus.py --bin target/release/openkrx --dir ~/corpus
```

It reads `*.krx` case-insensitively, in the named directory alone; add
`--recursive` to descend. Each file is passed to `inspect --json`,
`list --json` and `validate-structure --json` under a per-file, per-command
timeout (`--timeout`, 60 seconds by default). A directory inside the
repository tree is refused unless `--allow-in-repo` is given; the
conventional place for a corpus is the gitignored `/samples/`, and anywhere
outside the tree is better.

What it prints, and the whole of what it prints: the number of packages read;
the exit status per command; the `error.code` and `error.category` per
command; every structural check as check, outcome and code-or-rule; the
root-prefix class (`KRX/OCD/`, `OCD/`, none, other); the metadata file-name
casing class; the format-marker position class; the payload subdirectory
spelling class (`ID-<n>`, `ID<n>`, `ID_<n>`, other); the entry-count
histogram in the buckets 1-4, 5-9, 10-49 and 50+; and the runs that timed out
or produced no envelope.

What it never prints: a file name, an entry name, a path, a metadata value, a
hash, a timestamp or an identifier. Every label in the report is a fixed
string written in the script — a class name, a check name, an outcome, a rule
identifier or a stable dotted code the CLI already documents. A value read
out of a package is classified and then dropped; even an error is counted
rather than rendered, because an exception's text can carry the path it
failed on. Every refusal — an argument it cannot use, a directory that is not
there, a directory it cannot read — is one fixed sentence that quotes
nothing, and the run sits under a guard so that no traceback reaches a stream
either. The report's own header says as much, and it is for local reading: it
does not belong in an issue, a pull request, a commit or a comment.

The self-test is what keeps that promise honest:

```sh
python3 scripts/private-corpus.py --self-test --bin target/release/openkrx
```

It copies the five committed golden fixtures into a temporary directory under
deliberately loud file names, runs the script as a subprocess over them —
a real run's own streams, not the renderer's return value — and asserts the
bucket counts those fixtures must produce: five packages, the exit statuses
0, 3, 4, 6 and 8 where each fixture earns them, two `KRX/OCD/` prefixes
against one package with none, `root_prefix unresolved A19` once. It then
asserts that none of a canary set appears anywhere on stdout or stderr: the
planted file names, the temporary directory, the synthetic consignment
identifier, the synthetic attachment names and the metadata entry name. A
second run points `--dir` at a directory that does not exist and asserts that
the output is the fixed refusal and nothing else, canaries included, because
a refusal is where a path most easily escapes. A canary on either stream is a
privacy bug, and the run exits non-zero.

A finding is turned into a rule, never into a document. Cite the *class
count* in [profile.md](profile.md) — "N of M local packages carry the `KRX/`
prefix, and none carried another" — and let that sentence be the evidence the
rule moves on. The package that showed it is not named, not quoted, not
attached and not turned into a fixture: when a real package reveals a
behaviour worth testing, the fixture is written from scratch to exhibit the
property. Those rules bind the maintainer personally as well as the code, and
are stated in full in
[roadmap.md](roadmap.md#private-corpus-policy-for-maintainers).
