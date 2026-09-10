# Measured limits

The limit table in [architecture.md](architecture.md#limits) is the contract.
This document is one measurement of what sitting on that contract costs, taken
by `scripts/measure-limits.py` on one machine, on one day.

**These numbers are a measurement, not a guarantee.** They describe the
machine named below and nothing else: another CPU, another allocator, another
operating system or another build profile will produce different ones, and no
number here is a promise about any run anywhere. What the table *does* hold is
the right-hand column — every package built one step past a limit was reported
with the stable code that limit publishes, and every package built exactly on
a limit was accepted. That part is a property of this repository rather than
of this machine, and `scripts/measure-limits.py` fails when it stops holding.

| Fact | Value |
| --- | --- |
| Commit | `f988e768dd9dcd6a2ad5a53d733e8195bb92b590` |
| Date | 2026-09-10 |
| Operating system | Linux-6.12.107+deb13-amd64-x86_64-with-glibc2.41 |
| CPU | 13th Gen Intel(R) Core(TM) i9-13900 |
| Python | 3.13.5 |
| Peak-memory source | `os.wait4 (ru_maxrss for the measured child)` |

## What was measured

For each limit, the generator
`crates/openkrx-core/examples/limit_packages.rs` writes two synthetic
packages: one sitting exactly at the limit, one sitting one step past it.
Every one of them is driven through `inspect`, `validate-structure` and
`extract --json`. The wall-time columns are per command; the peak resident
size is the largest any of the three reached, because that is the number a
caller sizing a container cares about.

| Limit | Default | Package | Measured | inspect (s) | validate-structure (s) | extract (s) | Peak RSS | Result |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `Limits::max_entries` | 256 | at the limit | 256 entries | 0.00 | 0.00 | 0.01 | 14.3 MiB | accepted |
| `Limits::max_entries` | 256 | one step over | 257 entries | 0.00 | 0.00 | 0.00 | 14.3 MiB | `archive.over_limit.entries` from inspect, validate-structure, extract |
| `MetadataLimits::max_text_bytes` | 1048576 | at the limit | 1048576 bytes of text | 0.01 | 0.01 | 0.01 | 14.3 MiB | accepted |
| `MetadataLimits::max_text_bytes` | 1048576 | one step over | 1048577 bytes of text | 0.01 | 0.01 | 0.01 | 14.6 MiB | `metadata.over_limit.text_bytes` from inspect, validate-structure |
| `Limits::max_entry_decoded_bytes` | 33554432 | at the limit | 33554432 decoded bytes in one entry | 0.07 | 0.07 | 0.15 | 66.7 MiB | accepted |
| `Limits::max_entry_decoded_bytes` | 33554432 | one step over | 33554433 decoded bytes in one entry | 0.08 | 0.09 | 0.08 | 35.0 MiB | `archive.over_limit.entry_decoded_bytes` from inspect, validate-structure, extract |
| `Limits::max_total_decoded_bytes` | 134217728 | at the limit | 134217728 decoded bytes in total | 0.31 | 0.26 | 0.62 | 37.5 MiB | accepted |
| `Limits::max_total_decoded_bytes` | 134217728 | one step over | 134217729 decoded bytes in total | 0.26 | 0.26 | 0.26 | 20.0 MiB | `archive.over_limit.total_decoded_bytes` from inspect, validate-structure, extract |
| `Limits::max_compression_ratio` | 100 | at the limit | 100 decoded per compressed byte | 0.01 | 0.01 | 0.02 | 14.6 MiB | accepted |
| `Limits::max_compression_ratio` | 100 | one step over | 127 decoded per compressed byte | 0.03 | 0.03 | 0.03 | 14.6 MiB | `archive.over_limit.compression_ratio` from inspect, validate-structure, extract |
| `ExtractLimits::max_depth` | 16 | at the limit | 16 destination path components | 0.00 | 0.00 | 0.00 | 14.6 MiB | accepted |
| `ExtractLimits::max_depth` | 16 | one step over | 17 destination path components | 0.00 | 0.00 | 0.00 | 14.6 MiB | `extract.over_limit.depth` from extract |

"Peak RSS" is the whole process: the executable, its runtime and the bounded
buffer it reads the package into. `validate-structure` returning exit 4 is the
normal outcome for every package this repository can build — rule M13 is
undecided, so `consistent` is unreachable — and is not a finding.

## What these numbers are not

They are not a performance contract, a capacity plan or evidence about any
real package. They are not a conformance, validity or interoperability
statement: structural checking is not signature verification, and openKRX
performs no cryptography. The packages measured are synthetic originals
written by this repository, as [the fixture
policy](../tests/fixtures/README.md) requires, and none of them is a
conforming KRX file.

## Re-running it

```sh
python3 scripts/measure-limits.py
```

It builds the release executable and the generator, writes every package into
a temporary directory outside the repository, measures, rewrites this
document and removes the packages. Nothing is left behind and nothing large is
committed. `--compare` reports drift against a previous run of the same
document without changing the exit status; see
[testing.md](testing.md#measuring-the-limits).
