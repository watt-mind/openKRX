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

`SKILL.md` is written out by the packaged executable rather than copied from
`crates/openkrx-cli/skills/openkrx/SKILL.md`, so the document in the archive
is the one compiled into the binary in the same archive; the smoke job diffs
the two and fails if they ever disagree.

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
| `build` | Six matrix legs: builds `openkrx-cli` for the target, stages the archive contents, writes `SKILL.md` from the binary, packs the archive and its `.sha256`. |
| `smoke` | Six matrix legs: downloads that target's archive, checks its SHA-256, unpacks it, and runs the packaged binary. |
| `checksums` | Downloads every archive and writes one `SHA256SUMS` over all of them. |
| `attest` | Produces SLSA build provenance for every archive. Skipped on a dry run. |
| `release` | Creates the **draft** GitHub release with the archives, `SHA256SUMS` and the changelog section as its body. Skipped on a dry run. |

The smoke job is what stands between a build and a release. For each of the
six targets it verifies the archive checksum, unpacks the archive, checks that
`LICENSE`, `README.md`, `CHANGELOG.md` and `SKILL.md` are all present and
non-empty, diffs the packaged `SKILL.md` against `openkrx skill`, and then
runs:

- `openkrx --version`, asserting it names the version being released;
- `openkrx capabilities --json`, asserting `schema_version` is `1` and that
  the operation list is exactly `inspect`, `list`, `validate-structure`,
  `extract`;
- `openkrx skill | head -1`, asserting the skill's front matter opens;
- `openkrx validate-structure tests/fixtures/golden/consistent.krx --json`,
  asserting **exit status 4** and the `unresolved` summary.

Exit 4 is the documented outcome for that fixture: no check fails and some
profile rules cannot be decided from the sources. A 0 there would mean the
binary had started emitting a conformance verdict, which is a release-blocking
change, so the smoke job treats anything but 4 as a failure.

### 5. Publish the draft by hand

The run leaves a draft release. Nothing publishes it. Read the body, check the
asset list and the checksums, and publish it yourself:

```sh
gh release view vX.Y.Z
gh release edit vX.Y.Z --draft=false
```

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
| `security.yml` | push, pull request, weekly cron, dispatch | Gitleaks over the full history, `actionlint` over every workflow, the advisory scan, and CodeQL. |
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
