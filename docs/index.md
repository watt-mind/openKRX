# Documentation

Start with the [README](../README.md) for who the tool is for, its status,
and what it can do today. Every limit, stable code, exit status and JSON
shape is specified in [architecture.md](architecture.md), the canonical
reference.

| Document | Contents |
| --- | --- |
| [architecture.md](architecture.md) | Canonical reference: goals, what is not implemented, the crate shape and per-file module map, format scope, the parser safety model, the archive and metadata limits, the eleven structural checks, the command contract and JSON envelope, exit statuses, and the cryptographic, interpretation and integration boundaries. |
| [profile.md](profile.md) | The evidence: rules A1 to A22 and M1 to M15 extracted from the primary sources with section-level citations and an evidence class each, the unresolved essential rules, the conformance evidence that exists and the evidence that does not, and the recorded redistribution terms. |
| [conformance.md](conformance.md) | Every profile rule mapped to the module or check that implements it, the outcome it can produce, the test that holds it, and its status, ending with the gaps that block a conformance claim. |
| [codes.md](codes.md) | Every stable `archive.*`, `metadata.*` and `input.*` code with its category, meaning, the numeric fields its error carries, the exit status it classifies to, and the test that asserts it; kept in step with the sources by `scripts/check-codes.py`. |
| [references.md](references.md) | The source register: each primary document with its retrieval status, what it establishes, the sources that could not be retrieved, and the copyright terms recorded for each. |
| [research.md](research.md) | What was searched for and found, what was not found, the design decisions and their reasons, the language and MSRV decision, and the open evidence gaps. |
| [testing.md](testing.md) | Test layout file by file, how to run the suite and the coverage gate, what the truncation and mutation sweeps guarantee, the fixture policy, the two fuzz targets with their bounded smoke lane and what it does not prove, later compatibility testing, and the private-corpus pointer. |
| [work-packages.md](work-packages.md) | The dependency-ordered implementation specifications KRX-01 to KRX-07, each with owned paths, acceptance criteria, verification and the unknowns it must not resolve by assumption. |
| [roadmap.md](roadmap.md) | What is not yet implemented, the milestones with their status, unordered engineering items, the residual risks in the current state, and the private-corpus policy for maintainers. |
| [releasing.md](releasing.md) | The gates that must be satisfied before a first release, and why release automation is a separate change. |

Repository policies live at the root:

| Document | Contents |
| --- | --- |
| [README.md](../README.md) | Who openKRX is for, its status, what exists today, the quick start, and the documentation pointers. |
| [CONTRIBUTING.md](../CONTRIBUTING.md) | Branch model, commit conventions, local checks, fixture and compatibility rules, and completion criteria. |
| [SECURITY.md](../SECURITY.md) | Current surface, how to report a vulnerability, the threat model package support must satisfy, and the archive and metadata threat-model mappings. |
| [AGENTS.md](../AGENTS.md) | Scope and ownership per crate, the non-negotiables, the quality gates, and the documentation maintenance map. |
| [CHANGELOG.md](../CHANGELOG.md) | Keep a Changelog history; every behaviour-changing pull request adds an entry under `[Unreleased]`. |
| [tests/fixtures/README.md](../tests/fixtures/README.md) | Why no binary fixture is committed, and what a future one would have to record. |

Factory operation, which is not part of the public documentation set:
[master orchestrator instructions](orchestrator.md) and
[runner setup](factory.md).

## Maintenance map

Documentation is maintained with the change, not after it. A pull request
that makes one of these changes updates the listed documents **in the same
pull request**; a reviewer treats a missing update as an incomplete change.
`scripts/check-codes.py` enforces the error-code row automatically, and the
rest is a review obligation.

| Change kind | Documents that must change with it |
| --- | --- |
| Core parsing behaviour: what an archive or a document is accepted or refused for | [architecture.md](architecture.md) (the section describing that behaviour), [testing.md](testing.md) if a test file gains or changes coverage, [CHANGELOG.md](../CHANGELOG.md) |
| Limits: a default value, a new limit, or what a limit is enforced against | [architecture.md](architecture.md#limits) (the limit table and its rationale), [codes.md](codes.md) if a code is added, [CHANGELOG.md](../CHANGELOG.md) |
| Error codes: adding, renaming or retiring a stable code | [codes.md](codes.md) (enforced by `scripts/check-codes.py`), [SECURITY.md](../SECURITY.md) threat-model mapping, [conformance.md](conformance.md) if a rule's outcome changes, [CHANGELOG.md](../CHANGELOG.md) |
| Profile rule status: a rule becomes resolved, or a new rule or ambiguity is found | [profile.md](profile.md), [conformance.md](conformance.md), [architecture.md](architecture.md#structural-check-inventory) if a check's outcomes change, [references.md](references.md) if a new source settles it, [CHANGELOG.md](../CHANGELOG.md) |
| CLI contract: a command, flag, JSON field, or exit status | [architecture.md](architecture.md#command-contract-and-json-envelope) and [Exit statuses](architecture.md#exit-statuses), [codes.md](codes.md) if a code is added or its status changes, [README.md](../README.md) quick start, [roadmap.md](roadmap.md) if it closes a milestone, [CHANGELOG.md](../CHANGELOG.md) |
| Tests and fixtures: a new test file, a new sweep, or any committed fixture | [testing.md](testing.md), [tests/fixtures/README.md](../tests/fixtures/README.md) for a fixture, [conformance.md](conformance.md) if it changes which test holds a rule |
| Dependencies: adding, removing or replacing one | [architecture.md](architecture.md#crate-shape), [research.md](research.md) for the reason it was chosen, [CHANGELOG.md](../CHANGELOG.md) |
| Security controls: a check, a boundary, or a privacy rule | [SECURITY.md](../SECURITY.md), [architecture.md](architecture.md#parser-safety-model), [testing.md](testing.md) for the test that holds it, [CHANGELOG.md](../CHANGELOG.md) |
| Release process: gates, versioning, publication | [releasing.md](releasing.md), [CONTRIBUTING.md](../CONTRIBUTING.md) if the branch model changes, [CHANGELOG.md](../CHANGELOG.md) |
