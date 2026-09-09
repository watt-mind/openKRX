# Roadmap, residual risks, and maintainer policy

What openKRX does not do yet, the order in which that changes, the risks
that remain in the code as it stands, and the rules a maintainer follows if
a private corpus is ever used.

Nothing here relaxes the rule stated in
[architecture.md](architecture.md#boundaries): until the code for a milestone
exists, no openKRX output may claim or imply that an archive is a conforming
package, and no output may claim anything cryptographic at all. The rules
themselves are in [profile.md](profile.md), the open ones in
[Unresolved essential rules](profile.md#unresolved-essential-rules), and
what the code asserts from them in [conformance.md](conformance.md).

## Not yet implemented

The three reader commands expose the three library layers, so
`capabilities().operations` names them. Reading is the only package operation
that exists. The authoritative, code-level version of this list is
[architecture.md](architecture.md#not-yet-implemented).

- Extraction of any kind, and with it every filesystem control the security
  policy requires: no-clobber publication, path sanitisation, symlink and
  reparse-point defence, and interrupted-write cleanup.
- Package creation and deterministic writing.
- Signature handling, including `signatures.xml` (rule A7). openKRX
  performs no cryptography and is not planned to.
- Attachment interpretation of any kind.
- ZIP64, encryption, multi-disk archives, and compression methods other
  than stored and deflate. These are refused with stable codes, not
  deferred.
- Fuzz targets for either parser.
- A CLI agent skill. Now that a reader command ships there is a workflow to
  describe, and it is a separate change rather than part of KRX-04.
- Configurable limits from the command line. `Limits` and `MetadataLimits`
  are library values a Rust caller can tighten; nothing on the command line
  reaches them.
- Releases, published crates and installation instructions. Both crates set
  `publish = false`; see [releasing.md](releasing.md).

## Milestones

The specifications live in [work-packages.md](work-packages.md); this table
is their status. Each package is one scoped change with its own acceptance
criteria and verification.

| Milestone | Status | Delivered or planned |
| --- | --- | --- |
| [KRX-01: establish the first profile](work-packages.md#krx-01-establish-the-first-profile) | **Done** | [profile.md](profile.md): rules A1 to A22 and M1 to M15 with section-level citations, an evidence class per rule, the unresolved list, and recorded redistribution terms. |
| [KRX-02: bounded archive inventory](work-packages.md#krx-02-bounded-archive-inventory) | **Done** | `openkrx_core::archive::inventory`: a bounded, profile-agnostic ZIP reader with published limits, exact byte coverage, ambiguity refusal and 43 stable codes. No CLI exposure. |
| [KRX-03: metadata and structural validation](work-packages.md#krx-03-metadata-and-structural-validation) | **Done** | `openkrx_core::metadata::parse` and `openkrx_core::profile::check`: bounded `KER_META_V0_9`-shaped parsing and the eleven-check structural inventory, with each unresolved rule reported as `Unresolved(rule)`. No verdict, no CLI exposure. |
| [KRX-04: reader CLI and stable output](work-packages.md#krx-04-reader-cli-and-stable-output) | **Done** | `inspect`, `list` and `validate-structure` over the existing layers, with bounded input, eight stable exit statuses, a one-object JSON contract and content-free diagnostics. The first milestone that makes any of this usable without writing Rust; its output stays within [profile.md](profile.md) and reports observations, never a conformance verdict. |
| [KRX-05: protected extraction](work-packages.md#krx-05-protected-extraction) | Planned | `extract`, planned before any write, with the full filesystem threat model held by tests on every supported OS. |
| [KRX-06: deterministic profile writer](work-packages.md#krx-06-deterministic-profile-writer) | Planned, and blocked on evidence | `create`. Cannot be built while the container layout is unresolved: see [Known gaps](conformance.md#known-gaps). |
| [KRX-07: consumer contract and first release review](work-packages.md#krx-07-consumer-contract-and-first-release-review) | Planned | The openPapir integration contract, the openSzigno attachment handoff, and the first-release readiness review. |

KRX-05 is next: it is the first milestone that writes to a filesystem, so
the whole extraction threat model becomes live with it. KRX-06 is the one
that cannot start, whatever the engineering appetite, until a citable source
or an authoritative statement settles the layout.

## Engineering items

Unordered, and independent of the milestone sequence.

- **Fuzz targets** for the archive and metadata parsers, with a CI lane and
  a crash-to-regression-test rule. The truncation and mutation sweeps are
  the compensating control today; see
  [testing.md](testing.md#fuzzing-planned).
- **Mutation testing** of the check inventory, to find checks that pass for
  the wrong reason. Meaningful only once the inventory stops growing.
- **Property tests** for the ZIP reader's coverage arithmetic, generating
  structurally valid archives and asserting that exactly the declared bytes
  are claimed.
- **A compatibility-report format**, so that a strictness rule can be
  discussed against a specific producer rather than in the abstract. See
  [testing.md](testing.md#compatibility-testing-later).
- **Command-line limit overrides.** The reader commands enforce
  `Limits::DEFAULT` and `MetadataLimits::DEFAULT` with no flag reaching
  them, so a package above a ceiling is refused with an over-limit code and
  no way to reconsider it from the command line.
- **Benchmarks** for the inventory's peak memory and throughput, to hold the
  streaming claims in
  [architecture.md](architecture.md#peak-memory-and-streaming) as measured
  facts rather than as design intent.

## Residual risks in the current state

Stated plainly, because the tests that would remove them do not exist yet.

- **No agreement with a real service has ever been demonstrated.** No public
  sample archive and no independent implementation were found, so every
  synthetic fixture reflects openKRX's own reading of the primary sources.
  A rule read wrongly would be read wrongly by the fixture too. This is the
  largest risk and no amount of internal testing reduces it.
- **Nine of thirty-seven rules are undecidable from the sources.** Five are
  reported as `Unresolved`; four are not asserted at all. A consumer that
  renders `Unresolved` as a pass, or that treats
  `StructureSummary::Consistent` as acceptance, would be making a claim
  openKRX explicitly does not make.
- **Neither parser is fuzzed.** The sweeps cover truncations and one-byte
  mutations of valid inputs, which is a narrow neighbourhood. Nothing has
  explored inputs further away.
- **Strictness has never been measured against real archives.** Exact byte
  coverage, ambiguity refusal and the case-folded collision rule will
  refuse archives that a permissive reader opens. Whether real producers
  emit any such archive is unknown.
- **Deflate correctness is inherited.** `miniz_oxide` is the only place
  where openKRX trusts an external implementation on hostile bytes. Its
  output is CRC-checked and size-bounded, which limits the consequences but
  does not remove the dependency.
- **The limits are reasoned, not measured.** They were sized from what the
  format description implies, not from a corpus. A real package larger than
  a ceiling would be refused as over-limit, correctly by the contract and
  unhelpfully in practice, until the ceiling is revisited with evidence.
- **No filesystem code exists beyond reading one input, so none of the
  extraction risks are mitigated — they are absent.** The reader opens the
  path it was given and writes nothing anywhere. The moment KRX-05 starts,
  the whole extraction threat model in [SECURITY.md](../SECURITY.md) becomes
  live at once.
- **The exit-status classifier keys on a code's category segment.** The core
  error enums are `#[non_exhaustive]`, so a new code cannot be made to fail
  compilation in the command-line crate. A test over
  [codes.md](codes.md) closes that gap only as far as the catalogue is
  complete, which `scripts/check-codes.py` enforces.

## Private-corpus policy for maintainers

No private-corpus harness exists. If one is ever introduced, these rules
bind, and they bind the maintainer personally as well as the code.

- **Opt-in and local.** Excluded from public CI, off by default, and never
  reachable by a default `cargo test`.
- **Aggregate output only.** A report may contain counts and stable
  error-code buckets. Nothing else. No filename, path, entry name, metadata
  field, payload byte, certificate identity, signature, barcode,
  identifier, timestamp or hash — a hash of a real document is still a
  handle on a real document.
- **Nothing derived becomes a fixture.** A public fixture is never derived
  from a private package, redacted or otherwise. When a private package
  reveals a behaviour worth testing, the fixture is written from scratch to
  exhibit the *property*, and the private original is never referenced.
- **Nothing leaves the machine.** No private package, or any part of one, is
  attached to an issue, a pull request, a commit, a comment or a
  vulnerability report. Never upload a test package to a real government
  endpoint.
- **A finding is reported as a rule, not as a document.** "Entries with a
  `KRX/` prefix occur" is reportable; the archive that showed it is not.

These rules restate [SECURITY.md](../SECURITY.md#data-and-key-policy) and
[AGENTS.md](../AGENTS.md#non-negotiables) and do not weaken either.
