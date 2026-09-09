# openKRX

An open-source Rust library and command-line tool for Hungarian KRX
document packages: the ZIP-based container used in Hungarian
administrative correspondence.

**It reads; it does not write.** `openkrx inspect`, `openkrx list` and
`openkrx validate-structure` read a package locally and report what is in it.
Nothing is extracted, nothing is created, nothing is uploaded and nothing is
verified. There are no releases or published Cargo packages yet.

## Who it is for

People and tools that need to look inside a KRX package locally, on their
own machine, without uploading it anywhere and without a service account:
someone who received a package and wants to know what is in it, and
software that has to import one into a local workflow. openKRX owns the
container layer. It preserves attachment bytes and leaves their
interpretation to explicit downstream tools.

It is deliberately not a validator that says yes or no. Three primary
sources describe three different KRX directory layouts, and no public
sample archive or independent implementation was found, so openKRX reports
what it observed and reports an undecidable rule as undecided. See
[the rule-to-implementation map](docs/conformance.md).

## Status

| Layer | State |
| --- | --- |
| Bounded ZIP archive inventory (`openkrx_core::archive::inventory`) | Implemented, library only |
| Bounded metadata parsing (`openkrx_core::metadata::parse`) | Implemented, library only |
| Structural check inventory (`openkrx_core::profile::check`) | Implemented, library only |
| Reader commands: `inspect`, `list`, `validate-structure` | Implemented |
| Extraction planning (`openkrx_core::extract::plan`) and the `extract` command | Implemented |
| `create` | Not implemented |
| Signature handling of any kind | Not implemented, and not planned |

The three reader commands render the three library layers and add no rule of
their own; `extract` writes what the planner decided, and adds no rule of its
own either. The implementation sequence is in the
[roadmap](docs/roadmap.md).

## What exists today

`openkrx-core` takes byte slices and returns typed values or typed errors.
It performs no filesystem, clock, process or network access, and resolves
nothing external.

- **The archive inventory** reads any ZIP image within its limits and
  reports the entries it contains. It is profile-agnostic: it knows nothing
  about KRX. Eight limits are enforced against bytes the decoder actually
  produced rather than against declared sizes, every byte of the image must
  be claimed by exactly one declared structure, and input that admits two
  readings is refused rather than resolved.
- **The metadata parser** reads one `KER_META_V0_9`-shaped XML document.
  DTDs, non-predefined entities, stray processing instructions and non-UTF-8
  encodings are refused with their own codes, and five limits are counted
  while streaming.
- **The structural checks** relate the two: eleven named checks in a fixed
  order, each reported as a pass, a failure with a stable code, an
  unresolved rule, or not applicable.
- **The extraction planner** decides what an extraction would create, before
  anything is created: a symlink, a special file, an unsafe path component, a
  Unicode or case collision or an exceeded output ceiling refuses the whole
  package. It is a pure function and touches no filesystem.

`openkrx-cli` adds the only filesystem writing in the project, in `extract`,
and it is deliberately narrow: the destination must already exist and be a
real directory, nothing is ever overwritten, no symbolic link or reparse
point is followed out of the destination, no permission bit or timestamp is
copied, and a run that fails part-way removes everything it created and
nothing that was already there.

Every failure carries a stable dotted code — 102 of them — and a diagnostic
prints the code, an entry index and numeric limit values only, never an
entry name, document text, attribute value or filesystem path.

**Structural checking is not signature verification and not delivery
evidence.** openKRX performs no cryptography. Attachments, including `.es3`
dossiers, are opaque bytes: they are never opened, signed, or submitted
anywhere. `validate-structure` reporting `consistent` means only that no
check failed and none was left undecided.

## Quick start

Requires Rust 1.88 or newer. From a checkout:

```sh
cargo build --release -p openkrx-cli
target/release/openkrx list               package.krx
target/release/openkrx inspect            package.krx
target/release/openkrx validate-structure package.krx
target/release/openkrx extract            package.krx --into ./out
```

Each command takes one file, or `-` to read standard input, and each accepts
`--json`. `list` prints what the archive holds:

```text
index  method   compressed    declared     decoded  crc32     name
    0  stored           19          19          19  10b6eee7  mimetype
    1  deflate         449        1003        1003  067b8732  KRX/OCD/Metalayer/KULDEMENY_META.xml
    2  stored           19          19          19  4b26d730  KRX/OCD/Payload/ID-1/synthetic.pdf
```

`inspect` adds what the package declares about itself and how every
structural check came out. `validate-structure` prints the checks alone and
puts the reading in its exit status: `0` when nothing failed and nothing was
left open, `3` when a check failed, `4` when a rule could not be decided.
The full table is in
[the architecture reference](docs/architecture.md#exit-statuses).

```text
Structural summary: unresolved
0 of 11 checks failed and 1 could not be decided. This is not a statement
that the package is a valid or conforming KRX file.
```

`extract` writes the package's files into a directory you name. That
directory must already exist — `extract` never creates it — and it must be a
real directory rather than a symbolic link; it need not be empty. **Nothing
is ever overwritten:** if any file the package would create is already there,
the whole extraction is refused before anything is written, and the command
exits 9.

```text
entry         bytes  path
    0            19  mimetype
    1          1003  KRX/OCD/Metalayer/KULDEMENY_META.xml
    2            19  KRX/OCD/Payload/ID-1/synthetic.pdf

3 files written, 5 directories created, 1041 bytes.
```

While a run is in progress the destination holds `.openkrx-extract.partial`,
which is removed when it finishes, so a destination that still contains that
file was interrupted and its contents are incomplete. If a write fails
part-way, every file and directory the run created is removed again and
nothing that was already there is touched. No permission bits and no
timestamps are copied from the package, and no symbolic link, special file or
nested archive is ever created or unpacked. Extracting a file is not a
statement that it is authentic or safe to open.

`--json` writes exactly one object on stdout and leaves stderr empty on
success:

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

`verified: false` states the cryptographic boundary and is never `true`.
`ok` reports that stdout carries a report rather than a diagnostic; it is not
a judgement of the package, and it stays `true` when a check failed. Read
`validate-structure`'s `summary`, or the exit status, to act on the checks.

Every diagnostic carries a stable code; it carries the code, an entry index
and numbers, and never the input path, an entry name or a metadata value.

## Documentation

| Document | Contents |
| --- | --- |
| [docs/index.md](docs/index.md) | The documentation index, and the maintenance map saying which documents each kind of change must update. |
| [docs/architecture.md](docs/architecture.md) | Canonical reference: module map, format scope, parser safety model, limits, the check inventory, the JSON envelope, exit statuses, and the boundaries. |
| [docs/profile.md](docs/profile.md) | The format rules extracted from primary sources, with citations, evidence classes, and the rules that remain unresolved. |
| [docs/conformance.md](docs/conformance.md) | Every rule mapped to its implementation, outcome, test and status, ending with the gaps that block a conformance claim. |
| [docs/codes.md](docs/codes.md) | Every stable error code with its meaning, numeric fields and asserting test. |
| [docs/research.md](docs/research.md) | What was searched for, what was found and what was not, and why the design is what it is. |
| [docs/testing.md](docs/testing.md) | Test layout, how to run the suite and coverage, the sweeps, and the fixture policy. |
| [docs/roadmap.md](docs/roadmap.md) | Milestones with status, engineering items, and the residual risks in the current state. |

## Related projects

- [openSzigno](https://github.com/watt-mind/openSzigno) handles Microsec
  `.es3` dossiers. Its verification results concern the dossier it checked
  and can never be promoted to a statement about an enclosing package.
- [openPapir](https://github.com/watt-mind/openPapir) is the planned local
  correspondence workflow consumer: cases, imported packages, and receipts.

## Development

The workspace contains `openkrx-core` and `openkrx-cli`, both unpublished.
Pull requests target `develop`; `master` is reserved for stable releases.

```sh
bash scripts/check.sh
cargo build --release --locked
```

See [CONTRIBUTING.md](CONTRIBUTING.md), [SECURITY.md](SECURITY.md) and
[AGENTS.md](AGENTS.md). The project is licensed under the
[MIT license](LICENSE) and is independent of government service operators.
