# openKRX master orchestrator

Use these instructions in a Claude session started at the openKRX repository
root. A local launch copy may live at `tmp/orchestrator.md`; `tmp/` is
ignored by this repository. Read the file and carry the authorised manual
work queue through specification, implementation, verification, review and
merge to `develop`. Do not enable background services or event dispatch.

The repository currently provides only help, version and capabilities.
No KRX parser, writer, extractor or signature verifier exists. The work
packages describe future scope, not implemented functionality or live queue
state. openPapir is a separate consumer project; changes there require a
separate ticket and ownership assignment.

## 1. Establish the execution boundary

Read `AGENTS.md`, `README.md`, `CONTRIBUTING.md`, `SECURITY.md`,
`docs/architecture.md`, `docs/references.md`, `docs/roadmap.md` and
`docs/work-packages.md`. Inspect current CI/security workflows and repository
scripts rather than relying on a remembered baseline.

If installed, read and use the Factory work, ticket, specification and merge
skills plus their referenced execution protocol. Resolve their locations
through the current harness. This file supplies a self-contained fallback
for the workflow; it does not invent credentials or missing tracker rules.
Repository privacy and safety requirements override generic skill examples
that put private ticket identifiers or maintainer paths in public PRs.

Before claiming work:

1. Resolve the repository's Factory registry entry and configured issue
   tracker from the local maintainer environment. Confirm the Git remote,
   project mapping, status/label meanings and identity used for claims.
   Never print credentials, private configuration or local registry paths.
2. Confirm the registry dispatch mode is `report_only`. This run is explicit
   manual `/factory-work`, not an event-runtime dispatch. Confirm there is
   no active event-runtime claim or second coordinator for this repository.
   Do not change dispatch mode, start daemons, or enable services. If
   exclusivity cannot be established, stop before claiming and report it.
3. Check GitHub and tracker authentication with read-only requests. Resolve
   the actual authenticated identity without displaying tokens. Confirm PR
   and issue-state permissions needed by the authorised run.
4. Check the branch, tracked working-tree changes, existing worktrees,
   active PRs and current remote `develop` commit. Preserve existing work;
   never stash, reset or discard somebody else's changes. Fetch the remote
   base. Confirm current `develop` CI is green before landing new work.
5. Confirm Rust/MSRV availability, Cargo, Git, GitHub CLI, Python, Node/npm,
   cargo-machete, cargo-deny and actionlint. Coverage additionally needs
   cargo-llvm-cov and LLVM tools. Inspect `scripts/check.sh` for changes.
   Missing tools are an environment problem, not permission to skip gates.
6. Create a local run checkpoint under ignored `tmp/`, only after confirming
   that Git ignores it. Record scope, coordinator identity, concurrency and
   current claims. Keep this record private and do not commit it.

Only one coordinator claims and merges for this run. Start with one worker;
never exceed the registry `max_in_flight` limit (initially one);
use at most two simultaneous worker/reviewer agents if paths and resources
are independent. Keep compilation-heavy checks serial when memory or disk
pressure appears. Do not add artificial ports, databases or environment
files: this is a stateless Rust CLI repository.

## 2. Build a dispatchable queue

Query the configured tracker for unassigned `Todo` tickets labelled
`ai:agent-ready`, excluding held/blocked/escalated work. Read full tickets
and dependency state; title-only summaries are insufficient. Resolve exact
labels and statuses from the installed protocol when available.

Every dispatchable ticket must contain these five substantive sections:

- Problem & Context
- Acceptance Criteria
- Source File Pointers
- Owned Paths
- Verification Command

Confirm bounded scope, explicit non-goals, existing source pointers,
non-overlapping ownership, executable verification, and completed dependency
evidence. Proposed modules/tests must be identified as new, never cited as
existing behavior. Reject a vague checklist or a stale dependency as ready.
Move incomplete specifications to `Triage` with a concise explanation.

At foundation handoff, only the profile-discovery specification `KRX-01`
is ready. `KRX-02` through `KRX-07` are dependent Triage proposals. These
are public specification keys, not private tracker identifiers. Re-read
live state each run: refine dependent tickets against the landed APIs and
source evidence before promoting them. Do not mass-promote the roadmap.

For profile discovery, require primary-source citations, explicit unknowns,
redistribution evidence before vendoring, and independently usable synthetic
conformance evidence. An unresolved required rule blocks its conformance or
writer claim. Never fill a gap from a conversational example or private
submission. Reader/writer agreement alone proves no interoperability.

Show the selected queue with titles and owned paths. Continue within the
user's authorised cap without asking for routine implementation choices.
An empty eligible queue ends implementation dispatch; report the next
specification/dependency work clearly.

## 3. Claim before spawning

For each candidate, re-check dependencies and owned-path overlap against
what is actually running, then claim from the coordinator session. Under
the standard Factory protocol, assign to the authenticated self, move to
`In Progress`, and add `ai:in-progress` and `agent:claude-code`. Read the
issue back immediately and confirm ownership and state before any edits.
If the claim is lost or another active claim is discovered, skip the work.
Never overwrite another claim to win a race.

Create one isolated worktree and `codex/<public-slug>` branch from current
`origin/develop` per claimed ticket. Prefer an existing repository worktree
script if introduced; otherwise use standard Git worktrees in the configured
local workspace area. Do not publish private ticket IDs in branch names or
paths that GitHub will expose. Never edit the coordinator's dirty checkout.

Give each worker a self-contained assignment with the full private ticket,
acceptance criteria, exact worktree, branch/base commit, owned paths,
non-goals, dependencies, verification commands and these standing orders:

- You are not alone in the codebase. Do not revert others' changes. Stay in
  your assigned worktree and owned paths; ask the coordinator to resolve
  overlap before changing shared contracts or files.
- Implement one ticket only. Keep attachments opaque. Follow synthetic-data,
  secret, diagnostic-privacy and verification boundaries in `AGENTS.md`.
- Heartbeat the private tracker at phase changes and at least every
  20 minutes. Report evidence and new blockers without sensitive input data.
- Run the exact ticket Verification Command and repository checks. Never
  bypass failures, lower gates or expand scope to make a result green.
- Use Conventional Commits and a public-safe PR targeting `develop`.
  Include the concrete behavior, validation, limitations and public issue
  link if one exists. Keep private tracker linkage in the tracker only.
- Provide a structured private handoff: PR/commit, verification commands and
  results, files versus owned paths, review/UX status, risks and follow-ups.
  Then transition to `In Review` with `ai:needs-review`, removing
  `ai:in-progress`. Workers never merge or publish releases.
- On a blocker, return the exact missing evidence/decision/environment need;
  preserve useful work and let the coordinator update the private queue.

Use the installed skill's model routing where available. Give narrow
read-only exploration to a lightweight specialist; reserve implementation
and independent review capacity for the actual ticket. Do not spawn agents
only to wait for CI. Refill a freed worker slot only after fresh claim and
ownership checks; never exceed two agents or assume dependencies landed.

## 4. Verify and review independently

Run the ticket's exact verification command plus the repository baseline:

```sh
bash scripts/check.sh
cargo build --release --locked
cargo +1.88 check --workspace --all-targets --locked
cargo llvm-cov --workspace --locked --fail-under-lines 90
```

The current check script runs formatting, Clippy with warnings denied,
workspace tests, Rust documentation with warnings denied, cargo-machete,
cargo-deny, documentation links, file lengths, Markdown lint and actionlint.
The current workflows additionally cover three operating systems, release
CLI smoke, commit hygiene, secret scanning and CodeQL. Re-read the actual
workflow definitions if they change. Do not mistake local success for CI.

For a materially changed user flow, request an independent CLI/UX assessment
after verification. Supply the exact worktree and launch commands. Fix
in-scope usability/recovery findings; record a justified skip for pure
research or internal changes. No real correspondence in evidence artifacts.

Open PRs receive a cold, read-only reviewer distinct from their implementer.
Use `factory-merge-reviewer` when available, otherwise assign an independent
review agent. Give it the ticket, handoff, owned paths, current head commit,
acceptance criteria and security boundaries. It returns `MERGE`, `FIX` or
`ESCALATE` with ranked evidence; green CI does not replace diff review.

Wait for CI using bounded tooling such as `gh run watch --exit-status`.
After a failed run, delegate log investigation to `factory-ci-doctor`, or a
read-only diagnostic agent if unavailable. Provide the repository and run
ID; receive the culprit step, minimal offending lines, and `TICKET`, `ENV`
or `FLAKE` classification. Keep raw logs out of the coordinator's context.
A retry needs a reason; do not relabel a repeatable failure as flaky.

Fix in-scope findings in the same branch, rerun affected checks and obtain
review of the fix. Allow at most two fix/review rounds before escalating.
Any new head commit invalidates prior head-specific approval/check evidence.

## 5. Merge only within the boundary

Only the coordinator may merge, one PR at a time, and only to `develop`
under the user's standing authorisation for this run. Never target or merge
`master`/`main`, tag releases, publish crates or enable deployment services.

Before each merge, confirm the exact current PR head was reviewed, all
applicable CI and security checks are completed and green, dependencies are
landed, conflicts are resolved and public content respects privacy. Query
all checks, not only the branch-protection subset. Missing, cancelled or
pending expected checks are not success; intentionally conditional skipped
jobs require confirmation that their workflow condition does not apply.

Security-behavior changes require a human review decision before merging:
ZIP/XML acceptance or resource policy, traversal/symlink/no-clobber defenses,
privacy/secret handling, verification claims, credentials, or security CI
controls. Prepare the complete diff, tests and independent findings first;
then pause that merge with the concrete decision and evidence. Continue
independent already-authorised work when it is safe. Pure source discovery
that changes no security behavior can follow the ordinary review path.

After merging, resolve the actual merge commit and wait for all applicable
push workflows on `develop`, including release CLI smoke and security, to
pass. There is no deployed service smoke to invent. Only then mark the
private ticket `Done`, remove transient execution labels, and clean up its
merged branch/worktree without discarding unrelated changes.

If post-merge `develop` fails, stop further merges immediately. Delegate
failure diagnosis and prepare a scoped fix or revert with the same review
requirements. Report the failure and decision needed; never keep landing
work onto a known-red base.

## 6. Checkpoint, resume and stop correctly

Update ignored `tmp/` checkpoint data after claims, handoffs, reviews,
merges and blockers. Include the base/head commits, worktree, owned paths,
private claim reference, PR/run links, verification results, review status
and next action. Do not store tokens, private document data or raw logs.

On resume, treat the checkpoint as a hint. Re-read live claims, worktrees,
PR heads, dependencies and CI before acting. Confirm sole coordination
again; never spawn duplicate workers from a stale checkpoint.

Stop dispatch after two consecutive environment/build failures affecting
separate tickets; use diagnostic evidence to distinguish infrastructure
failure from ticket-specific defects. Also stop for lost coordination,
missing required auth, exhausted authorised ticket cap or no eligible work.
Escalate safety decisions and evidence gaps explicitly; do not silently
weaken requirements or mark unfinished tickets done.

Provide a concise final report: merged, PR open, blocked, escalated or
skipped work; verification and post-merge CI evidence; newly filed Triage
follow-ups; and the next required decision. Keep private identifiers in the
private operator channel and public descriptions free of internal details.

## Runner setup and communication

Read `docs/factory.md` for host registration and fresh-clone setup. The
portable `.factory.yaml` contains no private tracker routing or credentials.
Report blockers in the private tracker and the current operator session.
Do not send external chat, email or push notifications without explicit
operator authorisation.
