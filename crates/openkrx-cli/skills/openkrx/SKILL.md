---
name: openkrx
description: >-
  Inspect, list, structurally check and extract Hungarian KRX document
  packages (.krx, the ZIP-based OCD container used in Hungarian
  administrative correspondence) with the openkrx CLI. Use whenever a task
  involves a .krx file, a KÉR or Hivatali kapu consignment package, a
  KULDEMENY_META.xml metadata document, or getting the attachments out of
  one. openkrx reads locally, uploads nothing, and verifies nothing.
license: MIT
compatibility: Requires the openkrx CLI on PATH; run `openkrx --version`.
metadata:
  author: watt-mind
  version: "1.0"
  source: https://github.com/watt-mind/openKRX
---

openkrx is a command-line reader for KRX document packages: the ZIP-based
container Hungarian public bodies and Magyar Posta services move
correspondence in. It lists the archive entries, prints what the enclosed
`KULDEMENY_META.xml` declares about itself, runs a fixed inventory of
structural checks, and writes the package's files into a directory you name.
It opens no socket, keeps no cache, logs nothing, and performs no
cryptography of any kind.

It is deliberately not a validator. The public sources describing KRX
contradict each other on nine essential points, so openkrx reports what it
observed and reports an undecidable rule as undecided. Your job when using
it is to carry that distinction all the way through to the user.

## Rules that always apply

1. **Pass `--json` and read the envelope.** The human text is for people;
   only the JSON object is a contract. One object on stdout per run,
   diagnostics on stderr — with one exception, rule 2.
2. **A usage error (exit `2`) carries no envelope at all.** The argument
   parser rejects the command line before anything runs, so stdout is
   *empty* even with `--json` and stderr holds plain usage text. Check the
   exit status before you parse stdout: a blind `json.loads(stdout)` fails
   here, on exactly the mistakes you are most likely to make — a forgotten
   `--into`, a misspelled subcommand, a flag on `skill`. Treat the stderr
   text as opaque; there is no code to branch on, only status `2`.
3. **Branch on the exit status first, then on `error.code`.** Statuses and
   dotted codes are stable; sentences are not. Never parse a message, and
   treat a code you do not recognise as a failure rather than as a success.
4. **Never claim conformance, validity or authenticity.** No output of
   openkrx supports the words "valid KRX", "conforming", "genuine",
   "authentic" or "legally effective". `summary: "consistent"` is not any
   of those; see [The boundary, stated once](#the-boundary-stated-once).
5. **`verified` is `false` in every response, always.** It is a stated
   boundary, not a result field. If you ever find yourself explaining why it
   is false in a particular run, you have misread it: nothing is ever
   verified, because nothing cryptographic is ever done.
6. **Attachments are opaque bytes.** Payload files — PDFs, `.es3` dossiers,
   nested archives, anything — are attacker-controlled input. openkrx never
   opens, decodes, executes or recursively unpacks them, and neither should
   you without saying so explicitly to the user first.
7. **Everything stays local. Never upload a package or any part of it.** Do
   not send a `.krx` file, an extracted attachment, an entry name or a
   metadata value to any service, API or scratch host, and do not paste
   package content into a ticket or a chat log unless the user asked for
   exactly that.
8. **Reading is bounded on purpose.** The input cap is 64 MiB and the
   archive limits (at most 256 entries, a name of at most 255 bytes, and
   ceilings on decoded bytes) are fixed. There is no flag to raise any of
   them. A refusal under a limit is the tool working, not the tool failing.
9. **Exit status semantics differ per command.** `inspect` and `list` exit
   `0` whenever they produce a report, *even when a check failed*. Only
   `validate-structure` puts the structural reading in the status. Checking
   `$?` after `inspect` and calling the package clean is the single most
   likely way to get this wrong.

## The boundary, stated once

**Structural checks are not signature verification.** openkrx implements no
cryptography at all: no digest checking against `signatures.xml`, no XAdES,
no certificates, no timestamps. A package that openkrx read without
complaint may be forged, altered, or assembled by anyone. If a user needs to
know whether a signature holds, openkrx cannot answer and must not be quoted
as if it had.

**`consistent` is not conformance.** `validate-structure` reports one of
three words:

- `consistent` — no check failed and none was left undecided.
- `inconsistent` — at least one check failed.
- `unresolved` — nothing failed, but at least one check could not be
  decided from the sources.

`consistent` is a statement about the eleven checks openkrx runs, nothing
more. It does not mean the package is a valid KRX file, that a receiving
service would accept it, or that its contents are what they claim to be.

**Nine rules are unresolved, and openkrx will not guess them.** The primary
sources disagree or are silent on: `A19` (directory nesting and where
`mimetype` sits — three sources show three layouts), `A20` (the marker
entry's compression method and byte-exactness), `A21` (entry-name character
encoding and case sensitivity), `A22` (payload subdirectory naming: `ID-1`,
`ID1` and `ID_1` all appear), `M11` (an official metadata example that does
not validate against its own schema), `M12` (whether the metadata file is
`KULDEMENY_META.xml` or `kuldemeny_meta.xml`), `M13` (whether `MERET` is
kilobytes or bytes, and its rounding), `M14` (whether `ELHELYEZKEDES` is
archive-root-relative), and `M15` (which metadata fields a receiving service
actually requires).

A check whose answer depends on one of those reports
`outcome: "unresolved"` with a `rule` naming it. **`Unresolved(rule)` is
neither a pass nor a failure.** It means openkrx read the package, found the
relevant facts, and refused to invent the rule that would turn them into a
verdict. It is never an error in the package and never something the user
can fix. Of the eleven checks, only five rules can appear this way: `A19`,
`M11`, `M12`, `M13` and `M14`. The other four unresolved rules are simply
never asserted.

A package declaring an attachment almost always ends up `unresolved`,
because check `declared_size` cites `M13` whenever a `MERET` value exists.
That is the expected outcome for a real package, and exit `4` is not bad
news.

## Check the tool, and install this skill

```sh
openkrx --version
openkrx capabilities --json
```

`capabilities` is the cheap identification call: it takes no file, always
exits `0`, and returns `data.project`, `data.stage` and `data.operations`,
the list of package operations this build implements. Use it to confirm the
binary is openkrx and that the operation you are about to run exists in this
build, rather than assuming from the version number. `skill` is not listed
in `operations`: it operates on no package.

The binary carries this document, so installing the skill needs no checkout.
Save it where your harness looks for skills — `.claude/skills/openkrx/` for
one project, `~/.claude/skills/openkrx/` for every project,
`.codex/skills/openkrx/` for Codex:

```sh
mkdir -p .claude/skills/openkrx
openkrx skill > .claude/skills/openkrx/SKILL.md
```

`openkrx skill` writes this document to stdout and nothing else: no
envelope, no `--json`, no file argument, nothing on stderr, exit `0`. Adding
either an argument or a flag to it is a usage error and exits `2`.

If there is no openkrx on PATH, build it from a checkout of the repository
with `cargo build --release -p openkrx-cli`; the binary lands in
`target/release/openkrx`. There are no published releases yet, so do not
invent an installer command.

## Exit statuses and the envelope

| Status | Category | When it happens | What to do |
| --- | --- | --- | --- |
| `0` | `success` | The command produced its report. For `validate-structure`, nothing failed and nothing was undecided. | Read `data`. |
| `2` | `usage` | The arguments were rejected: unknown command, missing `FILE`, missing `--into`, unknown flag. Also `openkrx skill --json` or `openkrx skill FILE`. | Fix the command line. **No envelope is written**; nothing was read. |
| `3` | `structure_inconsistent` | `validate-structure` only: at least one check failed. | Report which checks failed, by `check` and `code`. |
| `4` | `structure_unresolved` | `validate-structure` only: nothing failed, at least one rule could not be decided. | Report the rules in `unresolved_rules`. Not a defect. |
| `5` | `input` | The input could not be opened or read, or it reached the 64 MiB cap. | Check the path, permissions, and that it is a file. |
| `6` | `package` | The package is malformed, truncated or ambiguous, or an entry name is unsafe. | Report it; a truncated transfer is worth re-fetching. |
| `7` | `unsupported` | The package uses a feature this reader does not implement: ZIP64, encryption, multi-disk, another compression method, a refused XML feature. | Say the package is not damaged and another reader may open it. |
| `8` | `limit` | A documented parsing or extraction limit was exceeded. | Report the `limit` and `observed` numbers. Do not retry. |
| `9` | `output` | `extract` only: the destination could not be used, or a write failed. | Read `cleanup`; nothing incomplete was left behind. |

There is no status `1`. Statuses `3` and `4` are successful runs whose
*finding* is negative: the report is complete and `ok` is `true`.

**Status `2` is the one status with no JSON at all.** Every other status,
success or failure, writes one envelope on stdout in `--json` mode. A usage
error is decided by the argument parser before a command exists, so there is
nothing for a `command` field to name and no code to carry: stdout is empty
and stderr carries clap's usage text. Read the status first, always.

**`ok` is about the command, not about the package.** It says only that
stdout carries a report rather than a diagnostic, and it stays `true` when a
structural check failed. Never turn `ok: true` into "the package is fine".
To act on a package, read `validate-structure`'s `summary`, or its exit
status.

A successful response:

```json
{
  "schema_version": 1,
  "ok": true,
  "command": "validate-structure",
  "data": { "...": "command specific" },
  "verified": false
}
```

A failed one:

```json
{
  "schema_version": 1,
  "ok": false,
  "command": "inspect",
  "error": {
    "code": "archive.unsupported.zip64",
    "category": "unsupported",
    "entry_index": 0
  },
  "verified": false
}
```

`error` carries `code` (the stable dotted string), `category` (the word for
the exit status), and, when the failure is scoped or numeric,
`entry_index`, `limit` and `observed`. It never carries the input path, an
entry name or a metadata value — deliberately, so that a diagnostic can be
logged safely. `extract` failures carry a `cleanup` object as well. The
complete code catalogue, with what each code means and which status it maps
to, is `docs/codes.md` in the openKRX repository; quote the code itself to
the user and look the meaning up there rather than guessing from the name.

`schema_version` is `1`. Adding a field does not raise it, so ignore fields
you do not recognise; if you ever read a `schema_version` you do not know,
stop rather than guess.

## Workflow

Run the steps in order. Each is cheap, each reads the file afresh, and none
of them writes anything except step 5.

### 1. Identify the tool

```sh
openkrx capabilities --json
```

Confirms the binary and lists the implemented package operations
(`inspect`, `list`, `validate-structure`, `extract`). Do this once per
session, not once per file. `verified` is `false` here too.

### 2. Inspect the package

```sh
openkrx inspect PACKAGE.krx --json
```

Exit `0` whenever a report was produced; a failing check does **not** change
that. `data` carries three parts:

- `observations` — `entry_count`, and, when a metadata document was located,
  `root_prefix` (the entry-name bytes before `Metalayer/`),
  `metadata_entry_name`, `metadata_entry_index`, and `marker`, the outcome
  of the `mimetype` check. A name that is not valid UTF-8 appears as
  `root_prefix_hex` / `metadata_entry_name_hex` instead, because `A21`
  leaves the encoding open and openkrx will not guess an encoding.
- `metadata` — present only when a document was located and parsed:
  `version` (`KRX_VERZIOSZAM`), `source_system`, `consignment_type`,
  `consignment_id`, the optional `reference_id`, `barcode` and `error_code`,
  `created_at` (verbatim text, never interpreted — openkrx has no clock),
  `test` and `test_present`, `declared_attachment_count`, and
  `attachments[]`.
- `checks[]` — the same eleven checks `validate-structure` reports.

Each `attachments[]` entry has `number`, `declared_path` (`ELHELYEZKEDES`
joined to `FAJL_NEV`), `resolution` (`resolved`, `prefix_variant` or
`missing`), `entry_index`, `declared_size_text` (verbatim), the optional
`declared_size_value`, and `observed_size`. **The declared and observed
sizes are printed side by side and are never compared**, because `M13`
leaves the unit open. Do not compare them yourself and do not report a
"mismatch". `prefix_variant` means the reference resolved only after
allowing for a different root prefix — worth mentioning, not a failure.

This is the only command that prints a declared metadata value, and only on
stdout, because it is what it was asked for.

### 3. List the archive entries

```sh
openkrx list PACKAGE.krx --json
```

Profile-agnostic: it says what the ZIP image holds and nothing about KRX. It
runs no structural check at all, and exits `0` whenever it produces its
listing. `data.entries[]` is in central-directory order, each with `index`,
`name` (or `name_hex` when the bytes are not UTF-8), `name_utf8_flag`,
`method`, `compressed_size`, `declared_size`, `decoded_size` and `crc32`.
`declared_size` and `decoded_size` are reported separately on purpose:
`decoded_size` is what the decoder actually produced.

Use `list` to answer "what is in this file" when the metadata is missing,
unparseable, or beside the point.

### 4. Read the structural checks

```sh
openkrx validate-structure PACKAGE.krx --json
```

This is the command whose exit status means something about the package:
`0` consistent, `3` a check failed, `4` a rule could not be decided. `data`
has `summary`, `checks[]` and `unresolved_rules[]`.

Every element of `checks[]` has `check` (a stable snake-case name) and
`outcome`, one of four:

| Outcome | Meaning | Extra field |
| --- | --- | --- |
| `pass` | The check held. | — |
| `fail` | The check did not hold. | `code`, a stable dotted code |
| `unresolved` | The sources do not settle the rule. | `rule`, e.g. `M13` |
| `not_applicable` | The input the check needs is absent. | — |

The eleven checks, in the order they always appear:
`metadata_location`, `metadata_file_name`, `root_prefix`, `marker_entry`,
`metadata_parse`, `schema_optional_fields`, `handling_instructions_form`,
`attachment_references`, `attachment_count`, `attachment_uniqueness`,
`declared_size`.

Read them like this:

- **Count the outcomes, do not average them.** One `fail` decides the
  summary; `unresolved` only decides it when nothing failed.
- **`not_applicable` is not a pass.** Checks after the first report it when
  no metadata document was located, and checks 7 to 11 report it when the
  document declares no dispatch, attachment or count. A package with a
  missing metadata document can therefore show a long list of
  `not_applicable` — that is one failure (`metadata_location`) plus its
  consequences, not eleven separate findings.
- **`unresolved_rules[]` lists each cited rule once**, in the order the
  checks first cite it. It is the list to name to the user when you explain
  why there is no verdict. It can only contain `A19`, `M11`, `M12`, `M13`
  and `M14`.
- **`summary` never becomes a verdict.** Say "no structural check failed",
  not "the package is valid".

Failures worth recognising: `metadata.missing` and
`metadata.ambiguous.multiple_candidates` (check 1),
`metadata.malformed.marker_missing`, `.marker_not_first` and
`.marker_content` (check 4, the `mimetype` entry),
`metadata.reference.missing_entry` (a declared attachment that is not in the
archive), `metadata.count_mismatch` (`MELLEKLETEK_SZAMA` disagrees with the
references listed) and `metadata.reference.duplicate`.

### 5. Extract into a directory that already exists

```sh
mkdir -p ./out
openkrx extract PACKAGE.krx --into ./out --json
```

`--into` must name a directory that **already exists**, is a real directory
rather than a symbolic link or a reparse point, and is best made empty by
you first. openkrx never creates the destination: creating one would turn a
typo into a tree. The directory need not be empty, but **nothing is ever
overwritten** — if any file the package would create is already there, the
whole extraction is refused before a single byte is written, with exit `9`.
Make a fresh empty directory each time and this class of failure disappears.

What extraction does and does not do: it writes exactly the files the
package declares, joined under the destination; it creates no symbolic link,
no special file, and unpacks no nested archive; it copies no permission bit
and no timestamp from the package. Extracting a file is not a statement that
it is authentic or safe to open.

On success, `data` carries `files_written`, `directories_created`,
`bytes_written`, `items[]` and `marker_removed`. Each element of `items[]`
has `entry_index`, `path` (relative to the destination, joined with `/`) and
`bytes`. `items[].path` is the only place outside `inspect` where a name
from the package reaches the output, and it appears on success only.
`marker_removed` is `true` in every successful report; assert it rather than
assume it. The destination you named is never echoed back — you already know
it.

**The marker.** While a run is in progress the destination holds
`.openkrx-extract.partial`. It is removed when the run finishes, so a
destination that still contains it was interrupted and its contents are
incomplete; openkrx refuses to extract into such a directory again
(`output.partial_marker_present`, exit `9`) until you clear it. If a write
fails part-way, every file and directory *this run* created is removed again
and nothing that was already there is touched — a failed `extract` reports
`cleanup` with `removed` and `left_in_place` counts. `removed: 0` means
nothing had been written at all, which is true of every refusal decided
before the first write. `left_in_place` above `0` is the one case where the
destination is not as you found it: say so.

### What to do on each failure category

| Exit | Read as | Action |
| --- | --- | --- |
| `2` | You built the command line wrong. | Fix it. There is no JSON and no code — only the status. Nothing was read, so do not report anything about the package. |
| `3` | A structural check failed. | Name the failing checks and their codes. Do not call the package invalid; say which check did not hold. |
| `4` | A rule could not be decided. | Name `unresolved_rules[]` and explain that the sources leave them open. Not a defect in the package. |
| `5` | The bytes could not be read, or exceeded 64 MiB. | Check path, permissions and size. Retrying the same path unchanged is pointless. |
| `6` | The package is malformed, truncated or ambiguous. | Report the code. `archive.malformed.eocd_missing` covers both a truncated archive and bytes that were never a ZIP at all, and the stderr sentence suggests re-fetching for both: check the file size and type before telling a user their download was corrupted. Nothing else here is retryable. |
| `7` | An unimplemented feature. | Say the package is not damaged and that another reader may open it. Never present this as the package being broken. |
| `8` | A documented limit was reached. | Report `limit` and `observed`. There is no flag to raise it; do not retry. |
| `9` | The destination could not be used, or a write failed. | Read `cleanup`; use a fresh empty directory, or clear a leftover marker. |

Never loop over variations of a package to see what changes. openkrx is a
reader, not an oracle, and a package is someone's correspondence.

## Reporting to the user

Say what was **observed**, in this order:

1. What the file is: entry count, whether a metadata document was located
   and where, the consignment type and `KRX_VERZIOSZAM` when parsed, and how
   many attachments were declared and how many resolved.
2. How the checks came out: how many passed, which failed and with which
   code, which were undecided and which rules they cite, and how many did
   not apply and why. Give the summary word and the exit status.
3. What was extracted: file and directory counts, byte total, where it went,
   and, on a failure, what `cleanup` reported.
4. Anything a human should look at: a `missing` attachment reference, a
   `prefix_variant` resolution, a `count_mismatch`, a name that had to be
   reported as hex, or a leftover marker.

Never say "valid KRX", "conforming", "verified", "authentic" or "signed".
The accurate forms are "no structural check failed", "openkrx could not
decide rule M13 because the sources leave the unit of `MERET` open", and
"nothing here was verified: openkrx performs no cryptography".

When a run ends in `4`, or when a check is `unresolved`, **name the rules**
and say in one clause what each leaves open. A user told only "unresolved"
will hear "broken"; a user told "the metadata file name casing (M12) is not
settled by the sources" hears the truth.

Do not paste metadata values — consignment identifiers, barcodes, reference
identifiers, attachment file names — into a summary, a log or a ticket
unless the user asked for exactly those values. Report counts and outcomes
by default. Diagnostics are already content-free; keep your prose that way
too.

## Quick reference

```sh
openkrx --version
openkrx --help
openkrx capabilities [--json]
openkrx inspect            <FILE|-> [--json]
openkrx list               <FILE|-> [--json]
openkrx validate-structure <FILE|-> [--json]
openkrx extract            <FILE|-> --into <DIR> [--json]
openkrx skill
```

| Command | Reads | Writes | Exit statuses |
| --- | --- | --- | --- |
| `capabilities` | nothing | stdout | `0`, `2` |
| `inspect` | one package | stdout | `0`, `2`, `5`, `6`, `7`, `8` |
| `list` | one package | stdout | `0`, `2`, `5`, `6`, `7`, `8` |
| `validate-structure` | one package | stdout | `0`, `2`, `3`, `4`, `5`, `6`, `7`, `8` |
| `extract` | one package | stdout and `--into DIR` | `0`, `2`, `5`, `6`, `7`, `8`, `9` |
| `skill` | nothing | stdout, no envelope | `0`, `2` |

Every command that reads a package takes exactly one input: a path, opened
exactly as written with no normalisation or globbing, or `-` for standard
input, read as binary. There are no limit-override flags, no configuration
file, no colour detection and no shell completions; the same arguments
produce the same bytes on every supported system.

The full reference is `docs/architecture.md` in the openKRX repository, the
code catalogue is `docs/codes.md`, and the evidence behind every rule id
used above is `docs/profile.md`.
