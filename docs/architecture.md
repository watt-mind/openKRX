# Architecture and CLI contract

## Current implementation

The edition-2024 Rust workspace targets Rust 1.88 and contains:

| Crate | Responsibility |
| --- | --- |
| `openkrx-core` | Current capability model; future package semantics |
| `openkrx-cli` | `openkrx` executable and human/JSON presentation |

Both crates use `publish = false`. No archive/XML parser or writer is
implemented. `openkrx --help`, `openkrx --version`, and
`openkrx capabilities [--json]` are the entire supported CLI surface.
Help and version use the argument parser's normal output. Successful
capability reporting exits with status 0; invalid CLI arguments exit with
status 2. Future I/O and format errors need their own reviewed contract.

In JSON mode, capabilities writes exactly one JSON object on stdout:

```json
{
  "schema_version": 1,
  "ok": true,
  "command": "capabilities",
  "data": {
    "project": "openKRX",
    "stage": "scaffold",
    "operations": []
  },
  "verified": false
}
```

`operations` lists implemented package operations; empty means none.
`verified: false` expresses the cryptographic boundary. Diagnostics belong
on stderr. Consumers should tolerate additional object fields. Incompatible
contract changes require an explicit schema-version decision.

## Planned package architecture

The core crate will accept bounded byte streams or injected readers and
return typed data/errors. It must not open paths, call clocks, launch
processes, or access the network implicitly. The CLI owns bounded file I/O,
argument handling, filesystem protection, and presentation.

Separate ZIP inventory, metadata parsing, profile validation, extraction
planning, and writing. A ZIP inventory can be useful without interpreting a
profile, but it must not be labelled a conforming KRX solely because the
extension or marker matches. Profile rules require cited evidence and must
keep unknown, malformed, and unsupported cases distinguishable.

Proposed commands, not yet available:

| Command | Intended result |
| --- | --- |
| `inspect` | Package/profile inventory and declared metadata |
| `list` | Attachment inventory with stable ordering |
| `validate-structure` | Checks against an explicitly supported profile |
| `extract` | Bounded, no-clobber extraction to a chosen directory |
| `create` | Deterministic package for a documented profile |

Before implementation, each command needs stable error codes, exit-status
categories, limits, privacy rules, and machine-output examples. Avoid bare
`valid` verdicts: specify `valid_structure` and the exact profile/checks.
Unknown required rules must prevent a claim of full conformance.

## Safety and determinism

All archive and XML boundaries in [SECURITY.md](../SECURITY.md) are required
before package features ship. Select concrete numeric limits through the
first reader work package and publish them here with maximum-memory and
streaming behavior. There are no implemented parser limits to list today.

The writer must use caller-supplied timestamps and deterministic ordering,
ZIP settings, names, and XML serialization. Equal inputs and settings must
produce equal bytes. Creation must enforce the reader's applicable limits
and preserve opaque attachment bytes exactly. No implicit signing occurs.

Filesystem behavior belongs to a reviewed output layer. Specify no-clobber
publication, rollback, symlink/reparse-point defense, and race assumptions
for each supported OS; do not copy ZIP entry names directly into file paths.

## Integration boundaries

openSzigno may produce or verify an `.es3` attachment. openKRX preserves its
bytes without interpreting its signatures. Calling an external verifier is
an explicit consumer action; its result cannot authenticate the package.

openPapir will consume package data for local cases and receipt association.
Case storage, workflow state, receipt authenticity, government credentials,
and delivery integrations stay outside openKRX. Creating a profile-shaped
archive does not establish that a government service accepts uploads of it.
