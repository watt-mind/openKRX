# Contributing

openKRX is a scaffold for a local KRX library and CLI. Start with the
[work packages](docs/work-packages.md); format research precedes parser
conformance and package creation. Discuss scope through a public issue
before a substantial change. Report vulnerabilities through
[SECURITY.md](SECURITY.md).

## Branches and commits

`develop` is the integration/default branch. `master` is the stable release
branch. Open PRs against `develop`, using one issue and one `codex/` branch
per task. Use Conventional Commits, for example:
`feat(core): add bounded archive inventory`.

A PR describes the concrete behavior change, links its public issue when
one exists, lists validation and remaining limitations, and updates relevant
documentation. Every behaviour-changing PR adds an entry under
`[Unreleased]` in [CHANGELOG.md](CHANGELOG.md), and updates the documents
the [maintenance map](docs/index.md#maintenance-map) lists for that kind of
change, in the same PR. Never expose internal tracker information or local
maintainer paths. Record unrelated follow-ups separately rather than
expanding scope.

## Local checks

Rust 1.88 is the minimum supported version; the workspace uses edition 2024.
Use the committed lockfile.

```sh
bash scripts/check.sh
cargo build --release --locked
```

Install hooks with `lefthook install` if Lefthook is available. Hooks support
the workflow; CI remains authoritative. CI runs formatting, Clippy,
documentation checks, tests on Linux/macOS/Windows, MSRV checks, dependency
policy and unused-dependency checks, coverage, and security scanning. The
bootstrap workspace line-coverage floor is 90%; parser and filesystem work
must add meaningful boundary coverage, not only raise that aggregate.

Keep dependencies minimal, justified, and compatible with the MSRV and MIT
project licence. Actions must be pinned to immutable commits. Changes to
security controls need tests showing why their guarantees still hold.

## Fixtures and compatibility

Use independently authored synthetic fixtures and document their intended
property and provenance in [tests/fixtures/README.md](tests/fixtures/README.md).
Never attach a real submission or a redacted derivative to an issue or PR.
For a compatibility request, provide a synthetic reproducer, expected
profile/version, a public source, and the expected behavior. Keep unsupported
profiles distinguishable from malformed input and resource-limit failures.

Do not vendor an XSD, manual, or sample from a third party until its
redistribution terms have been checked and recorded. Public accessibility
alone is not permission. See [references](docs/references.md).

## Completion criteria

Acceptance criteria pass; affected documentation is current; local checks
and required CI are green; privacy and security boundaries hold. A package
round-trip alone is insufficient evidence of external interoperability.
There is no package publishing or release automation in this foundation.
