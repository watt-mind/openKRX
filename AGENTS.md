# openKRX agent instructions

Read [README.md](README.md), [CONTRIBUTING.md](CONTRIBUTING.md),
[SECURITY.md](SECURITY.md), and [docs/architecture.md](docs/architecture.md)
before changing code. The CLI offers help, version, capabilities, the three
reader commands `inspect`, `list` and `validate-structure`, which render the
core crate's bounded archive inventory, bounded metadata parsing and
structural check inventory over one bounded input; `extract`, which
writes a package's files into a directory the caller names under a
no-clobber, no-link, undo-on-failure policy; `create`, which writes one
package from a strictly validated JSON manifest to a file that must not
already exist, and reads it back through the structural checks before
reporting success; `repack`, which edits an existing package into a new file
under the same output rules, preserving every attachment no edit names byte
for byte and refusing any package the writer cannot re-emit; and `skill`,
which writes the
embedded agent skill document to stdout outside the JSON envelope. A package
openKRX writes is structurally consistent with the documented layout and is
never a conforming one, and structural validation is not signature
verification: openKRX performs no cryptography, so no output means that
anything was verified. Never describe
planned operations as implemented.

## Scope and ownership

- `crates/openkrx-core/`: bounded ZIP and XML processing, the profile rules,
  extraction planning, and deterministic writing and repacking. No
  implicit filesystem, network, process, or clock access; inject I/O where
  needed.
- `crates/openkrx-cli/`: arguments, human/JSON output, bounded input and safe
  output handling. Keep package semantics in the core crate.
- `crates/openkrx-cli/skills/openkrx/SKILL.md`: the agent skill embedded in
  the binary and written out by `openkrx skill`. Update it in the same pull
  request as any change to a command, an exit status or the envelope.
- `docs/`: architecture, evidence, work package specifications, and roadmap.
- `tests/fixtures/`: independently authored synthetic data only.

Follow [docs/work-packages.md](docs/work-packages.md) in dependency order.
One scoped ticket, one `codex/` branch, one reviewable PR to `develop`.
Record acceptance criteria and verification before implementation. Existing
work may be concurrent: do not revert another contributor's edits. Assign
explicit file/module ownership when delegating independent work.

Public GitHub issues are intake; the maintainer execution queue lives in
the configured issue tracker. Resolve its configuration through the
maintainer environment before claiming work; never invent tracker settings.
Track follow-ups separately and keep the current work package bounded.
Do not publish internal tracker details or maintainer-local paths in public
issues, commits, or PRs.

## Non-negotiables

- Synthetic fixtures only. Never inspect, enumerate, print, hash, or publish
  real correspondence, private paths, titles, payloads, personal metadata,
  certificate identities, or signatures. Private-corpus reporting, if later
  authorised, is aggregate counts and stable error-code buckets only.
- Never commit secrets, `.env` files, private keys, or a complete PEM
  private-key armour line, even synthetic. Generate needed keys at runtime;
  never allowlist a secret-scanner rule.
- Package parsing and structural validation prove neither authenticity nor
  legal effect. No crypto or government submission belongs in this scope.
- Treat enclosed `.es3`, PDF, ASiC, and other documents as opaque payloads.
  Never execute attachments or recursively unpack them by default.
- ZIP/XML limits, no-clobber output, path sanitisation, and symlink handling
  are load-bearing requirements. Do not weaken them for compatibility.
- External schemas and specifications need an explicit redistribution
  licence before vendoring. Link sources and record evidence instead.
- No production readiness, complete-profile support, or service acceptance
  claims without documented implementation and independent evidence.

## Quality gates

Run `bash scripts/check.sh` and checks relevant to the change. The standard
Rust checks are:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --release --locked
```

CI also covers documentation, three operating systems, a minimum-supported-Rust
job pinned to 1.88, dependencies, coverage, a bounded fuzz lane, and security.
Read workflow files for the exact current gates.
Keep Rust files under the repository's enforced size limit; split by
responsibility. Use Conventional Commits. Update the CLI contract, security
model, evidence map, and roadmap when behavior changes. Do not add tests
that merely repeat trivial implementation details; exercise boundaries and
observable contracts.

## Documentation maintenance

Documentation is updated in the same PR as the change it describes, not
afterwards. The full table is the maintenance map in
[docs/index.md](docs/index.md#maintenance-map); the short form:

| Change | Update |
| --- | --- |
| Parsing behaviour | `docs/architecture.md`, `docs/testing.md`, `CHANGELOG.md` |
| A limit or its default | `docs/architecture.md` limit table, `CHANGELOG.md` |
| A stable code | `docs/codes.md`, `SECURITY.md` mapping, `CHANGELOG.md` |
| A profile rule's status | `docs/profile.md`, `docs/conformance.md`, `CHANGELOG.md` |
| The CLI contract | `docs/architecture.md`, `README.md`, `crates/openkrx-cli/skills/openkrx/SKILL.md`, `CHANGELOG.md` |
| Tests or fixtures | `docs/testing.md`, `tests/fixtures/README.md` |
| A dependency | `docs/architecture.md`, `docs/research.md`, `CHANGELOG.md` |
| A security control | `SECURITY.md`, `docs/architecture.md`, `docs/testing.md` |
| The release process | `docs/releasing.md`, `CHANGELOG.md` |

`scripts/check-codes.py` enforces the code row automatically: it extracts
every `archive.*`, `create.*`, `extract.*`, `input.*`, `manifest.*`,
`metadata.*`, `output.*` and `repack.*`
literal from `crates/*/src/**`
and fails when one is missing from `docs/codes.md`, or when that document
lists a code no source defines. It runs inside `bash scripts/check.sh`. The
other rows are a review obligation, and a missing update is an incomplete
change.

Every behaviour-changing PR adds an entry under `[Unreleased]` in
`CHANGELOG.md`. Never write "valid KRX", "conforming" or an equivalent as a
claim; those words appear only in negations and boundary statements, because
`docs/profile.md` lists unresolved essential rules.

## Factory orchestration

For an explicitly requested orchestration run, read
[the master orchestrator instructions](docs/orchestrator.md) and
[runner setup](docs/factory.md). Claim and re-read the private ticket before
editing, use one isolated worktree per ticket, and obtain independent review
before serial merges to `develop`. Only green post-merge CI permits `Done`.
The repository privacy rules override generic Factory examples that expose
private tracker identifiers in public branches, commits or PRs.
