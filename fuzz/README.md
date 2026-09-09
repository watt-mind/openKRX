# Fuzz targets

A separate `cargo-fuzz` package, deliberately **not** a member of the root
workspace: it needs a nightly toolchain and libFuzzer, while the workspace
itself stays on stable, and keeping it out means `cargo deny`,
`cargo llvm-cov --workspace` and the MSRV check never see these dependencies.

Two targets, each one call and no assertion beyond "the call returns":

| Target | Entry point |
| --- | --- |
| `inventory` | `openkrx_core::archive::inventory(data, &Limits::DEFAULT)`, then `entry_bytes` for every accepted entry |
| `xml_metadata` | `openkrx_core::metadata::parse(data, &MetadataLimits::DEFAULT)` |

Running them, reproducing an artifact and adding a target are documented in
[docs/testing.md](../docs/testing.md#fuzzing). The short form:

```sh
cargo +nightly fuzz build --fuzz-dir fuzz
cargo +nightly fuzz run --fuzz-dir fuzz inventory -- -max_total_time=30
```

`target/`, `corpus/` and `artifacts/` are local working directories and are
ignored by Git. No corpus is committed; see
[regressions/README.md](regressions/README.md) for what may be.

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
