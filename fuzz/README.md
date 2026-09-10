# Fuzz targets

A separate `cargo-fuzz` package, deliberately **not** a member of the root
workspace: it needs a nightly toolchain and libFuzzer, while the workspace
itself stays on stable, and keeping it out means `cargo deny`,
`cargo llvm-cov --workspace` and the MSRV check never see these dependencies.

Five targets. The first three are one call and no assertion beyond "the call
returns"; the last two additionally hold one invariant of this crate each.

| Target | Entry point |
| --- | --- |
| `inventory` | `openkrx_core::archive::inventory(data, &Limits::DEFAULT)`, then `entry_bytes` for every accepted entry |
| `xml_metadata` | `openkrx_core::metadata::parse(data, &MetadataLimits::DEFAULT)` |
| `structure` | `inventory`, then `openkrx_core::profile::check(&inventory, &MetadataLimits::DEFAULT)` |
| `extract_plan` | `inventory`, then `openkrx_core::extract::plan(&inventory, &ExtractLimits::DEFAULT)` |
| `create_round_trip` | a `PackageSpec` built from the bytes with `arbitrary`, through `openkrx_core::create::package(&spec, &Limits::DEFAULT)` |

The two invariants:

- `extract_plan`: every component of a produced plan is non-empty, is neither
  `.` nor `..`, and holds no `/` or backslash — a caller can join it blindly.
- `create_round_trip`: a package the writer produced reads back with no failing
  structural check and with every attachment byte-identical; a refusal reports
  a `create.*` code `docs/codes.md` catalogues, which the target reads out of
  that document with `include_str!` so the two cannot drift apart.

Running them, reproducing an artifact and adding a target are documented in
[docs/testing.md](../docs/testing.md#fuzzing). The short form:

```sh
python3 fuzz/seed.py
cargo +nightly fuzz build --fuzz-dir fuzz
cargo +nightly fuzz run --fuzz-dir fuzz inventory -- -max_total_time=30
```

[seed.py](seed.py) builds `corpus/<target>/` from the committed golden
fixtures under `tests/fixtures/golden/`: each `.krx` for the three targets
that take a whole package, the extracted `KULDEMENY_META.xml` for
`xml_metadata`, and that document concatenated with the payload members for
`create_round_trip`, which reads any byte string through `arbitrary`. It is
Python 3 and the standard library, it reads nothing outside the checkout, and
`--verify` asserts the corpus exists and prints the per-target counts and
nothing else. Both CI lanes run it before they fuzz.

`target/`, `corpus/`, `artifacts/` and `coverage/` are local working
directories and are ignored by Git. **No corpus is committed** — the seeds are
derived at run time, never checked in; see
[regressions/README.md](regressions/README.md) for what may be.

Two lanes run these targets. `.github/workflows/ci.yml` seeds the corpus and
runs each target for 30 seconds on every push and pull request;
`.github/workflows/fuzz.yml` is the weekly campaign — 20 minutes per target by
default, a `minutes_per_target` dispatch input, a coverage pass over
`inventory`, and the corpus and any crash artifacts uploaded for 14 days. It
is deliberately not a required check. Both are documented in
[docs/testing.md](../docs/testing.md#fuzzing).

`fuzz/Cargo.lock` is committed so a fuzz build resolves the same dependency
tree everywhere, and CI enforces it: `cargo fuzz` accepts none of cargo's
manifest flags, so the fuzz job resolves the graph with
`cargo +nightly metadata --manifest-path fuzz/Cargo.toml --locked` before it
builds, which fails on an absent or stale lockfile. Dependabot has its own
`cargo` entry for `/fuzz`; refresh the lockfile by hand with

```sh
cargo +nightly update --manifest-path fuzz/Cargo.toml
```

whenever `fuzz/Cargo.toml` changes, and commit the result with it.
