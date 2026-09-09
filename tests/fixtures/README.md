# Synthetic fixtures

No KRX fixtures are committed. Runtime CLI tests exercise only capability
reporting and argument handling.

The archive tests need no committed fixture either. Every archive they read,
valid and hostile alike, is generated at run time by the test-only synthetic
ZIP writer in `crates/openkrx-core/tests/support/`, which is an independent
original written for this repository and can emit contradictory headers on
purpose. Generating fixtures beats committing them: the intended property of
each archive is visible in the test that builds it, and no opaque binary has
to be trusted or re-derived. A binary fixture may only be added if a future
property genuinely cannot be generated, and must then be recorded below with
its provenance, property under test, and expected result.

Future fixtures must be independently authored synthetic originals, never
real submissions or redacted/modified derivatives. Record each fixture's
filename, creator/provenance, supported profile, property under test, and
expected result here. All contributors must have the right to license their
fixture under the accompanying [MIT licence](LICENSE).

Do not copy third-party example packages or schemas into this directory
without separately documented redistribution permission. Never include
personal metadata, real certificate identities, secrets, private keys, or a
complete PEM private-key armour line. Generate keys at runtime if required.
