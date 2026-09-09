# Release readiness

No releases, published crates, or binary distribution are configured by
this scaffold. Both workspace crates have `publish = false`.

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
6. Merge a reviewed `develop` to `master` release PR before tagging through
   that documented release process.

Do not remove `publish = false` or advertise installation instructions as
part of an unrelated feature ticket. Release automation and registry
publication require a dedicated, reviewable change.
