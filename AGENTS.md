# openKRX agent instructions

Read [README.md](README.md), [CONTRIBUTING.md](CONTRIBUTING.md),
[SECURITY.md](SECURITY.md), and [docs/architecture.md](docs/architecture.md)
before changing code. The repository is a scaffold: only help, version,
and capabilities exist. Never describe planned operations as implemented.

## Scope and ownership

- `crates/openkrx-core/`: library contract, then bounded ZIP/XML processing,
  profile rules, and deterministic writing. No implicit filesystem, network,
  process, or clock access; inject I/O where needed.
- `crates/openkrx-cli/`: arguments, human/JSON output, bounded input and safe
  output handling. Keep package semantics in the core crate.
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

CI also covers documentation, three operating systems, MSRV, dependencies,
coverage, and security. Read workflow files for the exact current gates.
Keep Rust files under the repository's enforced size limit; split by
responsibility. Use Conventional Commits. Update the CLI contract, security
model, evidence map, and roadmap when behavior changes. Do not add tests
that merely repeat trivial implementation details; exercise boundaries and
observable contracts.

## Factory orchestration

For an explicitly requested orchestration run, read
[the master orchestrator instructions](docs/orchestrator.md) and
[runner setup](docs/factory.md). Claim and re-read the private ticket before
editing, use one isolated worktree per ticket, and obtain independent review
before serial merges to `develop`. Only green post-merge CI permits `Done`.
The repository privacy rules override generic Factory examples that expose
private tracker identifiers in public branches, commits or PRs.
