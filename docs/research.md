# KRX research and design decisions

Last reviewed: 2026-09-09.

This is the record of what was looked for, what was found, what was not, and
why the code is shaped the way it is. It exists so that a later contributor
can tell a deliberate choice from an accident, and an evidence gap from an
oversight.

## Primary sources

The source register is [references.md](references.md): four Magyar Posta
documents (`KRX-SPEC`, `HK-2019`, `MKR-2.27`, `BKSZ-2.1`), the e-Papír help
page, and the unreachable SPOCS OCD catalogue entry, each with its retrieval
status and recorded copyright terms. The rules extracted from them, with
section-level citations and an evidence class per rule, are in
[profile.md](profile.md). Neither is duplicated here.

Two facts from that register shape everything below. First, exactly one
retrieved document states the container rules as such (`KRX-SPEC`, three
pages), and exactly one supplies a machine-checkable metadata grammar
(`KER_META_V0_9.xsd`, embedded in `HK-2019`). Second, none of the four
sources grants redistribution, so no schema, sample or document text is
committed to this repository.

## Existing implementations: what was searched for and found

The search covered public code hosting, package registries and the European
Commission's software catalogue, for a KRX reader, an OCD container
implementation, a published `.krx` sample, and any conformance suite or test
corpus.

**Nothing usable was found.** Stated precisely, so that a later contributor
does not repeat the search assuming it was cursory:

- No independent KRX reference implementation, in any language, was found.
- No public sample `.krx` archive was found. Every archive openKRX has ever
  read is one it generated itself.
- No conformance suite, test corpus or validator service for KRX was found.
- The upstream SPOCS OCD (Omnifarious Container for e-Documents)
  specification, which `MKR-2.27` names as the origin of KRX, could not be
  retrieved: the catalogue entry that document cites now redirects and
  returns HTTP 404. Its logical structure is described only second-hand.
  No licence terms could be recorded for it either.

This is an absence of evidence, not evidence that nothing exists. Producers
and receiving services clearly implement the format; their code is simply
not public. The consequence is recorded honestly in
[conformance.md](conformance.md#known-gaps): agreement with a real service
is unverified, and remains so until an official sample or an independent
implementation appears.

The one piece of independently usable conformance material is the schema and
the two `KULDEMENY_META.xml` samples embedded in `HK-2019`. They support
metadata-level checks only, and because they carry no redistribution licence
they are used as *structure* to re-express from scratch, never as content to
copy. `crates/openkrx-core/tests/metadata_evidence.rs` is that
re-expression; `tests/fixtures/README.md` states exactly what it does and
does not reproduce.

## Design decisions

### An original ZIP reader, not a general-purpose crate

A general ZIP crate is written to open archives that real tools produce. It
resolves ambiguity by convention — take the last end record, prefer the
central directory, tolerate a prefix so self-extracting archives work — and
those conventions are exactly the decisions this project needs to make
differently. Adopting one would have meant fighting its permissiveness at
every call site, and inheriting a dependency tree for a container format
whose entire supported surface is stored and deflate.

The reader is just under 1 600 lines across eight modules, forbids `unsafe`,
performs no I/O, and its rules are stated as tests. That is a size worth
owning. The trade is real and is written down here: openKRX will refuse
archives that a permissive reader opens, and a compatibility report against
a real producer may later force a documented, tested exception rather than a
general relaxation.

### Exact coverage, and refusing ambiguity

Every byte of an archive image must be claimed by exactly one declared
structure, and any input that admits two readings is refused rather than
resolved. Both rules exist for the same reason: a container that will later
carry legal correspondence must not be a place where a producer and a
consumer can be made to see different documents in the same bytes. Prefix
bytes, trailing bytes, unclaimed bytes, overlapping ranges, a second
terminating end record and a duplicate entry name are each a separate stable
code, so a compatibility discussion can be about one specific strictness
rather than about "the reader is strict".

Case-folded name collisions are refused for a narrower reason: rule A21
leaves entry-name case rules unresolved. With no rule to appeal to, treating
`Metalayer/` and `METALAYER/` as the same entry, or as two, would both be
inventions. Refusing the archive is the only option that invents nothing.

### `miniz_oxide` for deflate

Deflate is the one algorithm the format needs that is not worth writing.
`miniz_oxide` is pure Rust with no `unsafe` in the paths used here, has no
transitive dependencies of consequence, and exposes a streaming interface,
which is what the limit model requires: the decoder is driven one bounded
output buffer at a time, and every limit is checked against the bytes it
actually produced. A one-shot decompression API would have made
"enforce against decoded bytes, not declared sizes" impossible to honour.

### `quick-xml`, with the dangerous features refused above it

`quick-xml` is a pull parser: it emits events and resolves nothing on its
own. It opens no stream, fetches no external identifier, and has no
entity-expansion machinery to disable. That makes it the right base for a
document that arrives from an untrusted party.

The safety rules are implemented above it, in `metadata/scanner.rs`, at the
event boundary rather than deep in the grammar: a `<!DOCTYPE ...>`
declaration is refused before anything can be declared, a general entity
reference other than the five XML predefines is refused, a processing
instruction other than the XML declaration is refused, and a declared
encoding other than UTF-8 is refused rather than guessed at. Putting all
four in one place means the argument that no external resolution can happen
is a short one that a reviewer can check in a single file.

### `unicode-normalization`, for NFC and nothing else

The extraction planner has to decide whether two entry names would become the
same file. Two names that differ only in normalisation form — a precomposed
character against its decomposed sequence — are one file on macOS and two on
Linux, so the planner compares destination components after NFC and case
folding and refuses the package when two of them collide, rather than
renaming or skipping one. That comparison needs a normaliser, and writing one
means shipping the Unicode tables. `unicode-normalization` is
MIT OR Apache-2.0, has no transitive dependency of consequence, and is used
for NFC alone; nothing else in openKRX normalises anything.

### `clap_complete` and `clap_mangen`, so no second description exists

`completions` and `man` write documents that describe the command surface.
The alternative to generating them is committing a completion script and a
roff page and editing both whenever a subcommand, a flag or an exit status
changes — a second and a third description of the surface, each able to fall
behind the parser without anything failing. `clap_complete` and `clap_mangen`
remove that possibility: both render from `Args::command()`, the same `clap`
definition the binary dispatches on, at run time, so the documents belong to
the build that produced them.

Both are MIT OR Apache-2.0, maintained by the `clap` authors in the `clap`
repository against the same MSRV, and pulled in with default features off.
They add one transitive dependency between them, `roff`. Each is used by
exactly one command, in `crates/openkrx-cli/src/generate.rs`, and neither is
reachable from the core crate: no package semantics, no reading, no writing of
a package depends on either. The cost of that choice is that the bytes follow
the dependency version, which is why neither command has a golden case; see
[the golden output contract](testing.md#golden-output-contract).

### `rustix` on Linux, for `openat2` and nothing else

`extract` writes into a directory the caller names, and until this change it
checked each path and then created it. A principal with write access to that
destination can replace a component between the two steps, and the creation
follows the replacement out of the destination. No portable standard-library
call closes that window: `symlink_metadata` answers a question about the
past, and `O_NOFOLLOW` covers only the last component. `openat2(2)`, which
Linux 5.6 added, does close it — `RESOLVE_BENEATH` with `RESOLVE_NO_SYMLINKS`
makes the kernel decide at the moment of the operation.

Reaching that syscall means either raw bindings, which the workspace's
`unsafe_code = "forbid"` rules out, or a crate that has already written them.
`rustix` is that crate: `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT`,
which `deny.toml` accepts on the MIT arm; `default-features = false` keeps the
`linux_raw` backend, so no C library is linked and the only crates it brings
are `bitflags` and `linux-raw-sys`; and the two features it is given, `fs` and
`std`, are the filesystem calls and the standard types they are handed. It is
declared under `[target.'cfg(target_os = "linux")'.dependencies]`, so no other
platform's build or lockfile resolution sees it, and it is used from exactly
one module, `extract/linux_fd.rs`, which does nothing but make the calls.

The alternative considered and rejected was doing without: keeping the
check-then-create path everywhere and continuing to state the race as a
residual risk. It was rejected because the risk is one a caller cannot
mitigate except by controlling the destination directory, and the platform
most openKRX runs on can simply not have it.

### No vendored XSD

`KER_META_V0_9.xsd` is embedded in a posta.hu document that states only
"© Magyar Posta Zrt. Minden jog fenntartva!". Public accessibility is not
redistribution permission, so the schema is not committed, and neither is
any sample.

That has a design consequence, not just a licensing one: openKRX is not a
schema validator and does not pretend to be. `metadata/reader.rs`
re-expresses the shape rules M1 to M8 as code, each function citing the rule
it encodes. It is *schema-shaped parsing*, and the difference matters — a
real validator would reject the official `MKR-2.27` appendix example (rule
M11), so the reader deliberately accepts an absent `TESZT`, an absent
`MELLEKLET_LEIRASA` and a non-numeric `MERET`, records them, and lets check 6
report `Unresolved(M11)` rather than call an official example invalid.

### Locating the metadata document by observation

The obvious implementation is to look for the one path
`KRX/OCD/Metalayer/KULDEMENY_META.xml`. It is also unsupportable: three
primary sources describe three different directory layouts (A19) and two
sources spell the file name differently (M12). So the candidate set is
defined by *shape* — a last segment equal to
`KULDEMENY_META.xml` and a parent segment equal to `Metalayer`, compared
ASCII-case-insensitively, under any number of leading segments — and the
prefix and casing actually observed are reported as facts. The checks that
would have to decide (`root_prefix`, `metadata_file_name`) report
`Unresolved` citing the rule instead.

The same reasoning drives attachment resolution: a declared path resolves
byte-exactly, or it resolves only after swapping one plausible root prefix
for another, and the second case is `Unresolved(M14)` — never a pass, never
a failure. Two candidate metadata entries are an ambiguity, not a choice.

This is the single most consequential decision in the profile layer. It is
what lets openKRX read all three described layouts without asserting that
any of them is the right one.

## Language and MSRV

Rust, edition 2024, minimum supported version 1.88, `unsafe_code = "forbid"`
across the workspace.

The requirements that decided it: memory safety while parsing hostile input
without a garbage collector or a runtime; one static binary per platform for
a tool intended to run locally on a person's machine; a dependency set small
enough to audit (`serde`, `miniz_oxide`, `quick-xml` and
`unicode-normalization` in the core crate, with `clap` and `serde_json` added
by the CLI); and enough
type-system strength to make "an unresolved rule is not a failure" a
compile-time distinction rather than a convention. The sibling projects
openSzigno and openPapir are Rust, so a shared consumer contract stays in
one language.

1.88 is the floor because edition 2024 requires 1.85 and the workspace uses
language and standard-library items stabilised up to 1.88. It is checked in
CI as its own job, so raising it is a deliberate act with a changelog entry,
not a side effect of a dependency bump. Dependencies are held to the same
floor.

## Open evidence gaps

The full list, with what each would unblock, is
[conformance.md](conformance.md#known-gaps). In short: the container layout
(A19) and payload naming (A22) are contradicted between sources; the marker
entry's encoding (A20) and entry-name case rules (A21) are stated nowhere;
the metadata file-name casing (M12), the `MERET` unit (M13) and the meaning
of `ELHELYEZKEDES` as a path (M14) are contradicted or unstated; one
official example does not validate against the official schema (M11); and
what a receiving service requires beyond schema validity (M15) is named as a
requirement without being specified.

Two research actions would close most of it, and neither is code: an
authoritative statement from the format owner about the layout, and a
lawfully usable sample archive or an independent implementation to check
against. Until then, adding rules from conversational examples or from a
single observed package would be inventing a specification, which
[references.md](references.md#research-gate-before-implementation) forbids.
