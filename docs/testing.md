# Testing and quality gates

Everything openKRX asserts about a format rule is held by a named test. This
document says which test file holds what, how to run the suite and the
coverage gate, what the sweeps and the fuzz targets each guarantee, and the
rules for fixtures and for any private corpus.

## Test layout

Almost every test is an integration test, because the contract worth testing
is the public one: a byte slice goes in, a typed value or a stable code comes
out. The exceptions are the doctests in the public API documentation and four
`#[cfg(test)]` modules. The one in
`crates/openkrx-core/src/archive/inflate.rs` pins the CRC-32 implementation
against its published check value, the empty input, and chunk ordering — an
internal helper with no public surface to exercise it through. The ones in
`crates/openkrx-core/src/archive/kind.rs`,
`crates/openkrx-core/src/extract/paths.rs` and
`crates/openkrx-core/src/extract/collisions.rs` cover the host-system kind
mapping, the path-component classes and the path folding directly: two
component classes — `..` and a C0 control — are already impossible in an
accepted inventory, so the only way to hold the planner's own defence in
depth is to call it. The one in `crates/openkrx-cli/src/exit.rs` holds one
test per exit-status category and reads every code out of `docs/codes.md` to
assert that each classifies, with no head list of its own: a subprocess can
reach only the codes an archive can be built to produce, and
the contract covers every code the crates define.

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
| `crates/openkrx-core/tests/metadata_evidence.rs` | The independent-evidence layer: a synthetic re-expression of the *structure* of the two official sample documents (rules M9 and M10), parsed into the documented shape and resolved inside the documented layout. |
| `crates/openkrx-core/src/synthetic/` | Test-only synthetic writers, not tests, behind the non-default `synthetic-writer` feature so that the command-line tests can build the same archives. `mod.rs` builds ZIP images and can emit contradictory headers on purpose; `meta.rs` builds metadata documents from values written from scratch for this repository. `crates/openkrx-core/tests/support/mod.rs` re-exports them under the name the core tests use. |
| `crates/openkrx-core/examples/golden_fixtures.rs` | The generator behind the five committed packages under `tests/fixtures/golden/`, not a test: a canonical two-attachment package, the same package without the `KRX/OCD/` prefix, one declaring an attachment the archive does not hold, a truncated image and an over-limit one. Deterministic by construction; see [the golden output contract](#golden-output-contract). |
| `tests/golden/` | The byte-exact output contract: one directory per command run, holding its `cmd`, its exact `stdout` and `stderr` and its exit status, compared by `scripts/golden.py`. |
| `crates/openkrx-cli/tests/contract.rs` | The executable's argument surface by subprocess: the capability envelope's exact shape and its operation list, that the human output states the boundary rather than a verdict, that help names every command and the exit statuses, and that six kinds of argument error each exit 2. |
| `crates/openkrx-cli/tests/reader.rs` | Each reader command on a package that can be read, in both modes: the consistent package, the entry listing and its stable ordering, the declared document beside the observed archive, every check rendered as its own outcome, the two other layouts leaving A19 undecided, `-` on all three commands producing the same report as a file, a name that is not UTF-8 reported as bytes, an invisible character escaped, a long name cut with its remainder counted, and that a successful run writes nothing on stderr. |
| `crates/openkrx-cli/tests/extract.rs` | `extract`, the one command that writes, by subprocess against a destination each test owns and then reads back: a successful extraction with byte-identical payloads and nothing else created, the JSON report and its item order, a target file that already exists, a destination that is missing, a file, or a symbolic link, a marker left by an interrupted run, an entry named like that marker refused under the no-clobber code, a non-empty destination whose contents survive, a planner refusal keeping its own category and naming no path, a malformed and an unreadable package never reaching the destination, the usage errors, and — behind `cfg(unix)` — an ancestor symlink inside the destination, a pre-existing symlink at the leaf, and an injected write failure proving the cleanup pass removes exactly this run's files and leaves everything else. |
| `crates/openkrx-cli/tests/failures.rs` | Every exit-status category end to end: a failing check (3) for `validate-structure` only, malformed and truncated images (6), a ZIP64 extra field (7), an over-limit entry count with its numbers (8), a missing file, a directory and both an over-cap file and an over-cap standard input (5), one stderr line in human mode, and exactly one object on stdout in JSON mode even when the run failed. |
| `crates/openkrx-cli/tests/render_text.rs` | The human renderer's own lines, asserted whole rather than by substring: each attachment block with the entry it resolves to and its observed decoded size, a reference that resolves to nothing, one matched only after a root-prefix adjustment, the metadata entry with its index, the format-marker outcome, the failed and undecided check counts in the summary sentence, a check that does not apply reported as such rather than as undecided, the 200-character cut boundary at the limit and one past it, and the declared attachment count taken from the document rather than from the number of references. |
| `crates/openkrx-cli/tests/diagnostics.rs` | The rest of the diagnostic contract: the one content-free sentence each `output.*` code is given instead of its category's, for a destination that is not a directory, an occupied ancestor, an ancestor symbolic link and a failed write; the cleanup line a refusal reached before any write produces; and that a limit failure keeps its limit and observed numbers whichever command reported it. |
| `crates/openkrx-cli/tests/privacy.rs` | The content-free-diagnostic rule, by canary: a canary path segment, a canary entry name and a canary metadata value are searched for on stderr in every case and on stdout in every failing case, a declared value is shown to reach stdout for `inspect` alone, and `extract` is held to the same rule with its one documented exception — a successful report names the files it created, and still never the destination or a declared value. |
| `crates/openkrx-cli/tests/support/mod.rs` | Subprocess helpers and the synthetic packages the command tests read, not tests: a scratch directory that removes itself, a standard-input runner, and one builder per package shape. |

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

Three tests need a platform primitive that is not portable. The two symbolic
link tests and the injected write failure are behind `cfg(unix)`: Windows
cannot create a symbolic link without developer mode or elevation and cannot
create a junction without a reparse-point call this workspace forbids
(`unsafe_code = "forbid"`), and its ACL model does not make a directory
unwritable through one `set_permissions` call. The reparse-point rule is held
by the same code path — `FILE_ATTRIBUTE_REPARSE_POINT` in
`cli::extract::preflight::is_link` — rather than by a test on that platform,
and the injection test skips itself, loudly, when the process can write into
a read-only directory anyway. Everything else in `extract.rs` runs on all
three operating systems.

What the suite does **not** provide evidence about: package creation, which
does not exist; extraction under a concurrent writer at the destination,
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

What it pins, in 36 cases: `capabilities` in both modes; `inspect`, `list`
and `validate-structure` in both modes over all five committed fixtures —
which between them cover a consistent package, an undecided rule (A19), a
failed check, a malformed image and an exceeded limit, so exit statuses 0, 3,
4, 6 and 8 all appear; and `extract` in both modes into a fresh directory,
plus a second run into the same directory, which is refused under the
no-clobber rule with status 9.

A case directory holds `cmd`, one line of arguments, and optionally `setup`,
one line run first whose output is discarded — that is how the no-clobber
case gets a destination that is already populated. Two placeholders are
substituted: `{fixture}` becomes the repository-relative
`tests/fixtures/golden`, and `{outdir}` becomes an empty directory the case
owns. **Nothing is normalised.** The JSON envelope is serialised from Rust
structs in declaration order, so its key order is fixed; both renderers are
pure functions of the package; and `extract` reports destination-relative
paths and never the destination itself. The only absolute path any case is
given is its `{outdir}`, and the runner *fails* when that name appears on
either stream rather than masking it, because an output that carried it would
be a privacy bug in the executable, not a gap in the script.

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
is deliberately **not** a required check. A full workspace run rebuilds and
reruns the suite once per mutant. The seeding run — the whole workspace, both
crates, all 937 mutants, in one `cargo mutants --workspace --jobs 2`
invocation — took **7 minutes 39 seconds** of wall time on the machine it was
measured on. That is short because most mutants rebuild incrementally in under
a second and this suite runs in about one; the two mutants that make the
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
| `cli/extract/preflight.rs` `reparse_point` and its `FILE_ATTRIBUTE_REPARSE_POINT` mask | Windows-only code, not compiled on the platform the run was measured on, so no test could execute it. The Windows CI lane runs the same suite; the rule itself is `is_link`, and `docs/testing.md`'s note on the link tests explains why the reparse-point half is held by the code path rather than by a test. |
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

Both readers have a `cargo-fuzz` target. The package lives in
[fuzz/](../fuzz/README.md) and is **not** a member of the root workspace: it
needs a nightly toolchain and libFuzzer, and keeping it separate means
`cargo deny`, `cargo llvm-cov --workspace` and the MSRV check never see its
dependencies. `cargo machete` does scan the directory, and needs no exclusion:
`libfuzzer-sys` is reached through a normal `use`, so it is seen as used.

| Target | What it calls |
| --- | --- |
| `inventory` | `archive::inventory(data, &Limits::DEFAULT)`, then `entry_bytes` for every accepted entry, which is the only path that inflates data |
| `xml_metadata` | `metadata::parse(data, &MetadataLimits::DEFAULT)` |

Neither asserts anything about the result. The property under test is that the
call **returns** — accept or refuse — on any byte string, which is the same
property the sweeps hold one byte at a time.

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
```

Without `-max_total_time` a run continues until it is interrupted. The corpus
it accumulates is written to `fuzz/corpus/<target>/` and is ignored by Git;
delete it to start from nothing. Add `-rss_limit_mb=2048` on a memory-tight
machine, and run one target at a time.

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
to the CI lane. Keep the harness body to the single call: a harness that
asserts a result turns a behaviour change into a fuzzing failure, which is not
what this lane is for.

### The CI lane

The `Fuzz (build only)` job in `.github/workflows/ci.yml` runs on
`ubuntu-latest` for every push and pull request. It installs the nightly
toolchain and a pinned `cargo-fuzz`, builds both targets, then runs each for
**30 seconds** with `-rss_limit_mb=2048`, a 60-second total fuzzing budget.
Crash artifacts are uploaded when the job fails. The job names
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
refactor introduces — without adding minutes to every pull request. It is not
a campaign, and it finds nothing deep: the job passing is not evidence that a
reader is fuzz-clean, only that it survived a short bounded run from an empty
corpus. Deep fuzzing stays a local activity for now.

Outstanding, and none of it exists yet:

- **No seed corpus.** Each run starts from nothing, so a run rediscovers ZIP
  and XML structure from scratch. A corpus built at run time by the existing
  `tests/support/` writers — not committed as binaries — would let a short run
  start deep instead of shallow.
- **No scheduled long run.** Only the 30-second per-target lane exists; a
  weekly campaign with a persisted corpus is the natural next step.
- **No coverage measurement** of what the targets reach.
- **Only the two readers are fuzzed.** `profile::check` and `extract::plan`
  sit on top of them and have no target of their own.

Until those exist, the sweeps above remain the load-bearing compensating
control, and the residual risk stays recorded in
[roadmap.md](roadmap.md#residual-risks-in-the-current-state).

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

No private-corpus harness exists, and none may be added casually. If one is
ever authorised, the rules are in
[roadmap.md](roadmap.md#private-corpus-policy-for-maintainers) and
[SECURITY.md](../SECURITY.md#data-and-key-policy): opt-in, excluded from
public CI, and reporting aggregate counts and stable error-code buckets
only. No filename, path, metadata field, payload, certificate identity or
hash may enter a report, an issue, a commit or a pull request.
