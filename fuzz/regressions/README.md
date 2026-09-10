# Fuzz regressions

Minimised crash artifacts kept as permanent regression inputs, one directory
per target. Every directory is empty today apart from its `.gitkeep`: no
crash has been found, and no corpus is committed.

## What may be added here

Only a **fuzzer-generated, minimised** input, and only after
`cargo +nightly fuzz tmin --fuzz-dir fuzz <target> <artifact>` has reduced it.
Such an input is synthetic by construction — libFuzzer produced it from random
mutations — so it carries no correspondence, path, identity or payload from any
real submission. Nothing derived from a real package may be placed here, even
redacted, and the
[fixture policy](../../docs/testing.md#fixture-policy) applies unchanged.

## How a crash is handled

1. Minimise the artifact, and confirm it still reproduces.
2. Express it as a **generated construction** in the matching
   `crates/openkrx-core/tests/*_rejects_*.rs` file, using the synthetic
   writers, whenever it can be expressed that way. That named test, not the
   blob, is the regression evidence.
3. Commit the minimised blob here only when step 2 is genuinely impossible,
   and record in this file which target produced it and which stable code the
   input must now yield.
4. Only then fix the reader.

## Replaying

The `Fuzz (build only)` job in `.github/workflows/ci.yml` replays these
directories on every push and pull request — `-runs=0`, so each file is
executed once with no mutation — and fails on a crash. That is what makes a
blob committed here a permanent gate. A target whose directory holds nothing
but its `.gitkeep` is skipped and the job says so, which is what every target
does today; the step needs no edit when the first blob lands.

By hand, one target at a time:

```sh
cargo +nightly fuzz run --fuzz-dir fuzz inventory fuzz/regressions/inventory -- -runs=0
```
