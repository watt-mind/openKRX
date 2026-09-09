# Roadmap

## Foundation: present

Rust library/CLI skeleton, capability reporting, quality automation, and
public engineering documentation. No package operation is implemented.

## First usable reader

Establish one supported profile from evidence, then implement bounded ZIP
inventory, XML metadata parsing, `inspect`, `list`, and
`validate-structure`. Publish exact limits, unsupported cases, stable errors,
and the distinction between declared metadata and verified facts.

## Safe extraction

Implement an explicit extraction plan and protected filesystem output.
Deliver no-clobber behavior and adversarial path/resource tests on every
supported OS before advertising `extract`.

## Deterministic creation

Implement a writer for the documented profile after reader validation and
independent conformance evidence are available. Preserve opaque payloads,
require explicit timestamps, and verify deterministic output and rejection
of over-limit or inconsistent input.

## Consumer integration and release

Exercise openPapir integration using synthetic packages and a versioned
contract. Keep openSzigno integration at the attachment boundary. Release
only after the [readiness gates](releasing.md) are satisfied.

Implementation tasks and prerequisites live in
[work-packages.md](work-packages.md). Additional profiles, encrypted
transport envelopes, signature verification, and government delivery are
outside this initial roadmap. Any proposed expansion needs its own evidence
and scope review.
