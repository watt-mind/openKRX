# Releasing openKRX

**No releases, published crates, or binary distribution are configured yet.**
Both workspace crates set `publish = false`, and no tag exists. The automation
this document describes is in place and rehearsable, but a release remains a
human decision that the gates below still stand in front of.

Before the first release:

1. Complete and document the intended package operations and supported
   profile, with independent conformance evidence and explicit limitations.
2. Publish numeric limits, stable command/error contracts, and platform
   extraction guarantees backed by adversarial tests.
3. Pass required CI on the release commit, including dependency/security
   review and the minimum supported Rust version.
4. Review public fixtures and dependency licences, privacy boundaries,
   documentation, and vulnerability reporting. Move the `[Unreleased]`
   entries of [CHANGELOG.md](../CHANGELOG.md) under the new version
   heading; the changelog is the release notes.
5. Design a separate release change covering versioning, signed/provenance
   artifacts where supported, checksums, package contents, and smoke tests.
   **This is that change**: `.github/workflows/release.yml` and
   `scripts/release-notes.py`, described below. It builds, checksums,
   attests and drafts; it publishes nothing.
6. Merge a reviewed `develop` to `master` release PR before tagging through
   the documented release process below.

Do not remove `publish = false` or advertise installation instructions as
part of an unrelated feature ticket. Registry publication remains a separate,
reviewable change: nothing in this repository pushes to crates.io, to a
container registry, or to a package manager, and nothing here should be read
as a statement that openKRX is ready to be released.

## Readiness status

Assessed 2026-09-10, against the six gates above, for the openKRX side of
[KRX-07](work-packages.md#krx-07-consumer-contract-and-first-release-review).
The openPapir consumer contract is the other half of that package and is not
assessed here. This section is a statement of where the evidence stands; it
makes no release, creates no tag, and claims nothing beyond what the linked
proof supports. Every link is to a public file or workflow on the `develop`
branch, so a reader outside the project can check each row.

| Gate | Status | Evidence | What changes the status |
| --- | --- | --- | --- |
| 1. Operations and supported profile documented, with independent conformance evidence and explicit limitations | **Not met** | Operations, profile and limitations are documented: [architecture.md](https://github.com/watt-mind/openKRX/blob/develop/docs/architecture.md#not-yet-implemented), [profile.md](https://github.com/watt-mind/openKRX/blob/develop/docs/profile.md), [conformance.md](https://github.com/watt-mind/openKRX/blob/develop/docs/conformance.md). Independent conformance evidence does **not** exist: nine of thirty-seven rules are undecidable from the retrieved sources ([Known gaps](https://github.com/watt-mind/openKRX/blob/develop/docs/conformance.md#known-gaps), [Unresolved essential rules](https://github.com/watt-mind/openKRX/blob/develop/docs/profile.md#unresolved-essential-rules)), and the 2026-09-09 search found no public sample archive and no independent implementation ([Conformance evidence](https://github.com/watt-mind/openKRX/blob/develop/docs/profile.md#conformance-evidence)). | An authoritative statement from the format owner on A19–A22, or a lawfully usable sample archive or independent implementation. The two routes are the outreach questions in [research.md](https://github.com/watt-mind/openKRX/blob/develop/docs/research.md#open-evidence-gaps) and an aggregate-only run of the private-corpus harness ([private opt-in corpus check](https://github.com/watt-mind/openKRX/blob/develop/docs/testing.md#private-opt-in-corpus-check)); neither is code, and both are for a person. |
| 2. Published limits, stable command and error contracts, platform extraction guarantees backed by adversarial tests | **Partially met** | Limits are published as a contract ([Limits](https://github.com/watt-mind/openKRX/blob/develop/docs/architecture.md#limits)) and held by the [scaling guard](https://github.com/watt-mind/openKRX/blob/develop/docs/testing.md#the-scaling-guard). The command contract, the JSON envelope with `schema_version` 1 and the nine [exit statuses](https://github.com/watt-mind/openKRX/blob/develop/docs/architecture.md#exit-statuses) are pinned byte for byte by the [golden output contract](https://github.com/watt-mind/openKRX/blob/develop/docs/testing.md#golden-output-contract) and by `scripts/check-codes.py`. Adversarial tests exist: the truncation and mutation [sweeps](https://github.com/watt-mind/openKRX/blob/develop/docs/testing.md#sweeps), [property-based tests](https://github.com/watt-mind/openKRX/blob/develop/docs/testing.md#property-based-tests), five [fuzz targets](https://github.com/watt-mind/openKRX/blob/develop/docs/testing.md#fuzzing), and the link and reparse-point cases, including the Windows junction *and* symbolic-link tests. | One extraction guarantee is still stated rather than tested: the check-to-create race needs `openat2`-style resolution ([residual risks of the output layer](https://github.com/watt-mind/openKRX/blob/develop/SECURITY.md#residual-risks-of-the-output-layer)). Closing it, plus a fuzzing campaign with a persisted corpus rather than the bounded lane, moves this row. |
| 3. Required CI green on the release commit, including dependency/security review and MSRV | **Met per commit; unproven for a release commit that does not exist yet** | The nine required checks are named in [.factory.yaml](https://github.com/watt-mind/openKRX/blob/develop/.factory.yaml) and run in [ci.yml](https://github.com/watt-mind/openKRX/actions/workflows/ci.yml): format and lint, documentation, tests on three operating systems, the golden output contract, `Minimum supported Rust (1.88)`, `Dependencies` (`cargo deny` over [deny.toml](https://github.com/watt-mind/openKRX/blob/develop/deny.toml)), `Coverage` against the 90 % floor, plus the bounded fuzz lane. [security.yml](https://github.com/watt-mind/openKRX/actions/workflows/security.yml) adds Gitleaks over the full history, `actionlint` and CodeQL on every push and pull request, and its `Advisory scan (cargo-deny)` job on the weekly cron and on dispatch only — that job is the re-check of an unchanged dependency tree against newly published advisories, not the per-commit one; `Dependencies` in `ci.yml` runs the full `cargo deny check` on the release commit itself. | Nothing structural. The gate is satisfied only by a green required run on the exact commit that is tagged, so it is re-checked at release time, on the `develop` → `master` merge commit. [mutants.yml](https://github.com/watt-mind/openKRX/actions/workflows/mutants.yml) and [fuzz.yml](https://github.com/watt-mind/openKRX/actions/workflows/fuzz.yml) are weekly and deliberately not required; a red weekly lane is a reason to stop, not a blocking check. |
| 4. Public fixtures and dependency licences, privacy boundaries, documentation, changelog, vulnerability reporting | **Met** | Fixtures are synthetic originals with recorded provenance and a generator ([tests/fixtures/README.md](https://github.com/watt-mind/openKRX/blob/develop/tests/fixtures/README.md)) under their own MIT [licence](https://github.com/watt-mind/openKRX/blob/develop/tests/fixtures/LICENSE); dependency licences are allow-listed in [deny.toml](https://github.com/watt-mind/openKRX/blob/develop/deny.toml) and enforced by the required `Dependencies` check. Privacy boundaries: content-free diagnostics with canary tests ([command-line input layer](https://github.com/watt-mind/openKRX/blob/develop/SECURITY.md#threat-model-mapping-command-line-input-layer)), the [data and key policy](https://github.com/watt-mind/openKRX/blob/develop/SECURITY.md#data-and-key-policy) and the [private-corpus policy](https://github.com/watt-mind/openKRX/blob/develop/docs/roadmap.md#private-corpus-policy-for-maintainers). The documentation set is gated by `check-doc-links.py`, `check-codes.py` and markdownlint. [CHANGELOG.md](https://github.com/watt-mind/openKRX/blob/develop/CHANGELOG.md) is Keep a Changelog and is the release notes. Vulnerability reporting is GitHub private advisories ([Reporting](https://github.com/watt-mind/openKRX/blob/develop/SECURITY.md#reporting)). | Nothing outstanding. A new committed binary fixture, a dependency outside the allow-list, or a diagnostic that carries a value would reopen the row. |
| 5. A separate release change: versioning, provenance, checksums, package contents, smoke tests | **Met** | [release.yml](https://github.com/watt-mind/openKRX/blob/develop/.github/workflows/release.yml) is that change, reviewed on its own. `plan` re-reads the workspace version through `cargo metadata` and fails when the tag is not exactly `v<version>`; `build` produces six archives; `checksums` writes one `SHA256SUMS`; `attest` produces SLSA build provenance; `smoke` verifies each archive's checksum, contents and packaged `SKILL.md` and runs the binary on its own architecture. The whole path is rehearsable as a dry run, and the workflow publishes nothing. | The `attest` and `release` pair is the one part a dry run cannot exercise, because both have effects outside the run; the first real tag is what exercises it. |
| 6. A reviewed `develop` → `master` release pull request merged before tagging | **Not met** | No release pull request exists, no tag exists, and `master` has never received a release merge. The procedure is written down in [Cutting a release](#cutting-a-release) and the tag is created by hand. | Opening the `develop` → `master` pull request, having a human review it and merging it. This gate cannot be satisfied in advance; it is satisfied by doing it. |

### Gate 1, stated plainly

**Independent conformance evidence is absent, and no amount of further
internal testing produces it.** Three primary sources describe three
different container layouts and none supersedes the others, so A19 to A22
and M11 to M15 stay open; five of them are reported as `Unresolved` by a
named check and four are not asserted at all. No public sample `.krx`
archive, no independent implementation, no conformance suite and no PRONOM
record was found on 2026-09-09. Every fixture in this repository is
independently authored from openKRX's own reading of the documents, so a
rule read wrongly would be read wrongly by the fixture too.

The two routes out are the private-corpus report — the aggregate-only
harness a maintainer may run locally over real packages, which can turn a
class count into a rule but never into a fixture — and the outreach
questions in [research.md](research.md#open-evidence-gaps): one to the
format owner about the layout, one for the SPOCS D2.2 deliverable. Both are
for a person to make.

Therefore **a first release can only be a pre-release, labelled as such**.
It may ship a reader, a protected extractor, a deterministic writer and a
deterministic repacker whose behaviour is documented and tested; it may not
be presented as conforming, interoperable, secure or production-ready, and
nothing in the archives or in the release body may say that a package
openKRX reads or writes is acceptable to any service.

## A first pre-release, if one is cut

Nothing below is scheduled. It is the concrete shape a first release would
take, so that cutting one is a decision rather than a research task.

### The version

`0.1.0-alpha.1`, replacing the current `0.1.0-dev.0` in the
`[workspace.package]` table of the root `Cargo.toml`. `publish = false`
stays: no crates.io publish, no registry credential and no installation
instructions are part of this, and lifting `publish = false` remains a
separate reviewed change with its own `cargo-semver-checks` decision.

Under [SemVer](https://semver.org/spec/v2.0.0.html) §9 and §11 a version
carrying a pre-release identifier has *lower* precedence than the normal
version it precedes, so `0.1.0-alpha.1 < 0.1.0`, and dot-separated
identifiers are compared left to right — `0.1.0-alpha.1 < 0.1.0-alpha.2 <
0.1.0-beta.1 < 0.1.0`. The alphanumeric comparison also means
`0.1.0-alpha.1` sorts *below* the placeholder `0.1.0-dev.0` it replaces,
because `alpha` precedes `dev` in ASCII order. Nothing has ever been
released, no consumer resolves either string and neither has a tag, so that
ordering has no effect on anyone; it is recorded here so that no one later
reads the bump as an increase.

The tag is `v0.1.0-alpha.1`, and it must equal `v` followed by the workspace
version exactly. The `plan` job reads the version back through
`cargo metadata` and fails the run before anything is built when the two
disagree, so a mistyped tag stops the release rather than shipping archives
whose file names contradict the binaries inside them.

### What moves in the changelog

The whole `[Unreleased]` section moves under one new
`## [0.1.0-alpha.1] - <date>` heading, keeping its `### Added`,
`### Changed`, `### Removed` and `### Fixed` subsections in place — that is
every entry currently there, from the profile and the bounded inventory
through the reader commands, protected extraction, the deterministic writer
and `repack`, to the testing and release machinery. The changelog is the
release notes, so that section becomes the draft release body verbatim; a
new empty `[Unreleased]` heading is left above it.

Adding a version heading also ends the documented concession described in
[The release notes script](#the-release-notes-script): once any version
section exists, `release-notes.py --check X.Y.Z` no longer accepts
`[Unreleased]` as a substitute.

### What the run would produce

Exactly what [What ships](#what-ships) lists, and nothing else: six archives
(`openkrx-0.1.0-alpha.1-<triple>.tar.gz` for the five Unix targets and
`.zip` for `x86_64-pc-windows-msvc`), each containing the binary, `LICENSE`,
`README.md`, `CHANGELOG.md` and the `SKILL.md` the packaged binary wrote;
one `SHA256SUMS` over all six; a SLSA build-provenance attestation per
archive in GitHub's attestation store; and a **draft** GitHub release
carrying the archives, `SHA256SUMS` and the changelog section as its body.
No crate is published anywhere.

### The human steps

Copied from [Cutting a release](#cutting-a-release); nothing here is new
procedure.

1. On a branch off `develop`, set `version = "0.1.0-alpha.1"` in
   `[workspace.package]`, run `cargo update --workspace` so `Cargo.lock`
   follows, move the `[Unreleased]` entries under the new heading, and
   confirm the notes with `python3 scripts/release-notes.py 0.1.0-alpha.1`.
   Open the pull request against `develop` and let CI pass.
2. Open the release pull request from `develop` to `master`, review it, and
   merge it. This is gate 6.
3. Tag `master` after the merge, by hand:

   ```sh
   git fetch origin
   git tag -a v0.1.0-alpha.1 origin/master -m "openKRX 0.1.0-alpha.1"
   git push origin v0.1.0-alpha.1
   ```

4. Watch the run: `plan`, six `build` legs, six `smoke` legs, `checksums`,
   `attest`, `release`. The smoke job asserts, among the rest, that
   `validate-structure` over the consistent fixture still exits 4.
5. Publish the draft by hand. The tag carries a hyphen, so the workflow has
   already marked the draft as a pre-release; verify the flag rather than
   setting it:

   ```sh
   gh release view v0.1.0-alpha.1   # confirm isPrerelease: true
   gh release edit v0.1.0-alpha.1 --draft=false
   ```

   Until that command is run, no user-visible release exists. The release
   body is the changelog section, so the limitations a reader needs — the
   nine unresolved rules, the absence of any conformance claim — have to be
   legible there before the draft is published.

## What ships

One human action starts a release — pushing a tag — and one ends it —
publishing the draft. Everything in between is
`.github/workflows/release.yml`, which is hand-maintained rather than
generated, so the file a reviewer reads is the thing that runs.

| Artefact | Produced by | Where it lands |
| --- | --- | --- |
| `openkrx-<version>-<triple>.tar.gz` / `.zip` | `release.yml`, `build` | Draft release assets |
| `SHA256SUMS` over every archive | `release.yml`, `checksums` | Draft release assets |
| SLSA build provenance for every archive | `release.yml`, `attest` | GitHub attestation store |
| Release notes | `scripts/release-notes.py` reading `CHANGELOG.md` | The draft release body |

Each archive unpacks to one directory, `openkrx-<version>-<triple>/`,
containing:

| File | Where it comes from |
| --- | --- |
| `openkrx` (`openkrx.exe` on Windows) | `cargo build --release --locked -p openkrx-cli --target <triple>` |
| `LICENSE`, `README.md`, `CHANGELOG.md` | The repository at the tagged commit |
| `SKILL.md` | `openkrx skill`, run against the binary being packaged |
| `completions/openkrx.{bash,zsh,fish,ps1,elv}` | `openkrx completions <shell>`, run against the binary being packaged |
| `man/openkrx.1` | `openkrx man`, run against the binary being packaged |

`SKILL.md`, the five completion scripts and the manual page are all written
out by the packaged executable rather than copied from the sources, so every
document in the archive describes the binary in the same archive; the smoke
job regenerates `SKILL.md`, the bash script and the page and fails if any of
the three disagrees with what was packaged. The completion scripts and the man
page are generated by `clap_complete` and `clap_mangen` from the CLI's own
`clap` definition, so their exact bytes follow those dependency versions and
are not pinned by a golden case.

Targets built for every release:

| Target | Runner | Native |
| --- | --- | --- |
| `x86_64-unknown-linux-gnu` | `ubuntu-latest` | yes |
| `x86_64-unknown-linux-musl` | `ubuntu-latest` | yes (static; runs on the glibc host) |
| `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` | yes |
| `x86_64-apple-darwin` | `macos-15-intel` | yes |
| `aarch64-apple-darwin` | `macos-latest` | yes |
| `x86_64-pc-windows-msvc` | `windows-latest` | yes |

Every target is built on a runner of its own architecture. That is a
deliberate choice over `cross` or `cargo-zigbuild`: no emulator, container or
cross linker is in the trust path, and — more importantly — the smoke job can
execute the binary it just packaged on every target, so no archive reaches a
release without having been run. GitHub's arm64 Linux runner is free for
public repositories, which is what makes the `aarch64-unknown-linux-gnu` leg
native rather than cross-compiled.

## Cutting a release

### 1. Prepare the version on a branch off `develop`

Set the new version in the `[workspace.package]` table of the root
`Cargo.toml`, run `cargo update --workspace` so `Cargo.lock` follows, and move
the `CHANGELOG.md` `[Unreleased]` entries under a new `## [X.Y.Z]` heading with
its date. Confirm the notes the workflow will use:

```sh
python3 scripts/release-notes.py X.Y.Z
```

Open the pull request against `develop` as usual and let CI pass.

### 2. Open the release pull request from `develop` to `master`

`master` is the stable branch and is what the tag points at. Open a pull
request from `develop` to `master`, review it, and merge it.

### 3. Tag `master` after the merge

```sh
git fetch origin
git tag -a vX.Y.Z origin/master -m "openKRX X.Y.Z"
git push origin vX.Y.Z
```

The tag is created by a human, by hand, and nothing else in this repository
creates one. The workflow's first job re-reads the workspace version through
`cargo metadata` and fails the run if the tag is not exactly `v<version>`, so
a mistyped tag stops the run before anything is built.

### 4. Watch the run

`release.yml` runs, in order:

| Job | Does |
| --- | --- |
| `plan` | Reads the workspace version from `cargo metadata`, checks it against the tag, decides whether this is a dry run, and extracts the release notes from `CHANGELOG.md`. |
| `build` | Six matrix legs: builds `openkrx-cli` for the target, stages the archive contents, writes `SKILL.md`, the five completion scripts and the manual page from the binary, packs the archive and its `.sha256`. |
| `smoke` | Six matrix legs: downloads that target's archive, checks its SHA-256, unpacks it, and runs the packaged binary. |
| `checksums` | Downloads every archive and writes one `SHA256SUMS` over all of them. |
| `attest` | Produces SLSA build provenance for every archive. Skipped on a dry run. |
| `release` | Creates the **draft** GitHub release with the archives, `SHA256SUMS` and the changelog section as its body, marked as a pre-release when the tag carries a hyphen. Skipped on a dry run. |

The smoke job is what stands between a build and a release. For each of the
six targets it verifies the archive checksum, unpacks the archive, checks that
`LICENSE`, `README.md`, `CHANGELOG.md`, `SKILL.md`, the five files under
`completions/` and `man/openkrx.1` are all present and non-empty, diffs the
packaged `SKILL.md`, bash completion script and manual page against what the
packaged binary emits, and then runs:

- `openkrx --version`, asserting it names the version being released;
- `openkrx capabilities --json`, asserting `schema_version` is `1` and that
  the operation list is exactly `inspect`, `list`, `validate-structure`,
  `extract`, `create`, `repack`;
- `openkrx skill | head -1`, asserting the skill's front matter opens;
- `openkrx completions <shell>` for all five shells, asserting each writes a
  non-empty script, that an unknown shell exits 2 with an empty stdout, and
  that `openkrx man` opens a roff page with `.TH openkrx 1`;
- `openkrx validate-structure tests/fixtures/golden/consistent.krx --json`,
  asserting **exit status 4** and the `unresolved` summary.

Exit 4 is the documented outcome for that fixture: no check fails and some
profile rules cannot be decided from the sources. A 0 there would mean the
binary had started emitting a conformance verdict, which is a release-blocking
change, so the smoke job treats anything but 4 as a failure.

### 5. Publish the draft by hand

The run leaves a draft release. Nothing publishes it. Read the body, check the
asset list and the checksums, confirm the pre-release flag is what the tag
implies, and publish it yourself:

```sh
gh release view vX.Y.Z
gh release edit vX.Y.Z --draft=false
```

The `release` job passes `--prerelease` whenever the tag contains a hyphen,
which is exactly the SemVer pre-release form: `v0.1.0-alpha.1` is created as
a pre-release draft and `v0.1.0` is not. The flag is set at creation rather
than left to the publisher, because a draft published without it is a stable
release from the moment the button is pressed, and nothing a user's tooling
already fetched would notice it being corrected afterwards. It stays a
verification step for the human, not an action: `gh release view` reports
`isPrerelease`, and the `release` job prints the same field as its last line.

Until that command is run, no user-visible release exists. If the run produced
something you do not want to ship, delete the draft instead; the tag can stay
or be removed with `git push origin :vX.Y.Z`.

Verifying an archive as a user would:

```sh
sha256sum --check --ignore-missing SHA256SUMS
gh attestation verify openkrx-X.Y.Z-x86_64-unknown-linux-gnu.tar.gz \
  --repo watt-mind/openKRX
```

## What provenance does and does not say

`actions/attest-build-provenance` produces a SLSA build-provenance
attestation: a statement, signed by GitHub's Actions OIDC identity through
Sigstore, that these exact bytes were produced by this workflow, from this
commit, on a GitHub-hosted runner.

It is **not** a signature by a maintainer, and no maintainer key exists. It
says nothing about whether the binary is correct, safe, or free of defects,
and it is not a code-signing certificate: macOS Gatekeeper and Windows
SmartScreen are unaffected by it. What it lets a user do is refuse an archive
that did not come out of this repository's release workflow.

## Rerunning a failed job

Every job is idempotent up to the artifact names it writes, so a failed matrix
leg can be re-run from the run page ("Re-run failed jobs") without recutting
the release. The build and smoke legs need only their own target; `checksums`,
`attest` and `release` re-download whatever the run has.

The `release` job refuses to run if a release for the tag already exists,
rather than adding assets to it, so a re-run after a partially successful
`release` job needs the draft deleted first:

```sh
gh release delete vX.Y.Z --yes
```

Deleting a draft release does not delete the tag. If a change to the workflow
file itself is needed, the fix has to reach `master` and the tag has to be
moved, because a tag-triggered run reads the workflow from the tagged commit.

## What each workflow does

| Workflow | Trigger | Does |
| --- | --- | --- |
| `ci.yml` | push and pull request on `develop`/`master`; dispatch | Format, lint, docs, tests on three operating systems, the golden output contract, MSRV, dependency policy, coverage and the bounded fuzz lane. Also builds a release binary and smokes `--help`, `--version` and `capabilities --json`. |
| `security.yml` | push, pull request, weekly cron, dispatch | Gitleaks over the full history, `actionlint` over every workflow, and CodeQL, on every trigger; the advisory scan on the weekly cron and on dispatch only. |
| `mutants.yml` | weekly cron; dispatch | Mutation testing with per-crate floors. Deliberately not a required check. |
| `release.yml` | push of a `v*` tag; dispatch with `dry_run`; pull request changing `release.yml` or `release-notes.py` | Everything in [Watch the run](#4-watch-the-run). Creates a draft release and nothing else, and on anything but a `v*` tag creates nothing at all. |

`release.yml` is hand-maintained. There is no generator, no install step to
re-apply and no generated block to preserve: edit the file, run `actionlint`,
and rehearse with a dry run.

Every `uses:` in it is pinned to a full commit SHA with the version in a
trailing comment, as [CONTRIBUTING.md](../CONTRIBUTING.md) requires. Where the
same action is already used by `ci.yml` the SHA is the same one, so a bump
moves both files together.

## Rehearsing with a `workflow_dispatch` dry run

The whole build and smoke path runs on demand, from any branch, without
creating a tag, a release or an attestation:

```sh
gh workflow run release.yml --ref <branch> -f dry_run=true
gh run watch "$(gh run list --workflow=release.yml --limit 1 \
  --json databaseId --jq '.[0].databaseId')" --exit-status
```

A pull request that changes `.github/workflows/release.yml` or
`scripts/release-notes.py` rehearses the same way automatically. The path
filter is deliberately just those two files: a rehearsal costs six native
builds and six smoke jobs, which is far too much to spend on every ordinary
code pull request, and `workflow_dispatch` above already covers rehearsing
against arbitrary code. That trigger is also what makes a *new* release
workflow rehearsable at all: GitHub registers a `workflow_dispatch` workflow
only once it exists on the default branch, so `gh workflow run release.yml`
answers `404` until the change adding it has merged to `develop`. The two
paths run the same jobs with the same `dry_run` outcome.

`dry_run` defaults to `true`. On a dry run:

- `plan` reads the version from `cargo metadata` and takes the notes from
  `[Unreleased]`, since no version section exists yet;
- `build` and `smoke` run in full, on all six targets;
- `checksums` writes `SHA256SUMS`;
- `attest` and `release` are skipped.

Those two jobs — the only ones with effects outside the run — are guarded
twice over: by the `dry_run` value `plan` computed, *and* by a direct
`startsWith(github.ref, 'refs/tags/v')` test on the trigger. The second guard
is a fact about how the run started, so no later change to `plan`'s logic and
no trigger added to the file later can arm them by accident.

The archives, the checksums and the extracted notes are uploaded as workflow
artifacts, so a rehearsal produces exactly the files a release would, on the
run page, and nothing anywhere else.

Passing `-f dry_run=false` does **not** cut a release. The `plan` job fails
the run with an explicit message: a release is cut by pushing a tag, which is
the only path that has a tag to check the version against. That is deliberate,
and it keeps the release-creating path behind an action a human takes in
`git`.

The one part of the release path a dry run does not exercise is the `attest`
and `release` pair, because both have effects outside the run: a permanent
entry in the attestation store and a release object. Their inputs are the
artifacts the dry run does produce.

## The release notes script

`scripts/release-notes.py` slices one section out of `CHANGELOG.md` using only
the standard library. The changelog is the release notes; nothing generates
prose from commits.

| Invocation | Does |
| --- | --- |
| `release-notes.py X.Y.Z` | Prints the `## [X.Y.Z]` section. Exits 1 if it is missing or empty. |
| `release-notes.py --unreleased` | Prints the `## [Unreleased]` section. What a dry run uses. |
| `release-notes.py --check X.Y.Z` | Validates that section and prints nothing. |
| `release-notes.py --check --unreleased` | Validates `[Unreleased]`. |

`--check` with a version has one documented concession, which exists because
nothing has been released yet. While `CHANGELOG.md` holds **no version section
at all**, `--check X.Y.Z` accepts the `[Unreleased]` section for any version
and says so on stderr. The concession disappears the moment a first version
heading is added, and it never applies to the printing forms: the `plan` job
runs `release-notes.py "$version"` on a tag, which always requires the real
section, so a tagged release still cannot be drafted with notes nobody wrote.

## Required secrets

None beyond the automatic `GITHUB_TOKEN`.

There is no `CARGO_REGISTRY_TOKEN`, no registry credential, no signing key and
no organisation-level configuration to do before a release. The `release` job
uses `GITHUB_TOKEN` with `contents: write` to create the draft; the `attest`
job uses `id-token: write` and `attestations: write` for the OIDC exchange.
Every other job runs with `contents: read`.

If a future change does need a registry token, it is a change to
`publish = false` and belongs in its own reviewed pull request, not in this
workflow.

## Semver checks

Deferred until `publish = false` is lifted.

Neither crate has a published version, so there is no baseline for
`cargo-semver-checks` to compare against and no downstream consumer whose
build a breaking change could break. The JSON envelope is versioned separately
by its `schema_version` field, which is `1`; the rules for raising it are in
[CHANGELOG.md](../CHANGELOG.md), and the envelope itself is held byte for byte
by the golden output contract described in [testing.md](testing.md), which is
the compatibility gate that does exist today.

When a crate is first published, add `cargo-semver-checks` against the
previous release to `ci.yml` and record the decision here.
