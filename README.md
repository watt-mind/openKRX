# openKRX

An open-source Rust library and CLI for Hungarian KRX document packages.

**Status: scaffold.** The executable reports its capabilities; it does not
read, validate, extract, or create KRX files yet. No releases or published
Cargo packages are available. See the [roadmap](docs/roadmap.md) for the
implementation sequence.

## Purpose

openKRX will provide local, bounded package inspection and creation for
people and automation. Planned commands are `inspect`, `list`,
`validate-structure`, `extract`, and `create`.

KRX is a ZIP-based document container used in Hungarian administrative
workflows. Published service documentation describes OCD metadata and
payload directories, but does not establish one complete, universal profile
for every producer. The initial research must identify the exact supported
profile before the project claims conformance. See
[format research and references](docs/references.md).

**Structural validation is not signature verification or delivery evidence.**
The tool will treat attachments, including `.es3` dossiers, as opaque bytes.
It will neither sign documents nor submit them to government services.

## Try the scaffold

Requires Rust 1.88 or newer. From a checkout:

```sh
cargo run -p openkrx-cli -- --help
cargo run -p openkrx-cli -- --version
cargo run -p openkrx-cli -- capabilities --json
```

The capabilities response is one JSON object, shown formatted here:

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

An empty operations list means no package operation is implemented.

## Related projects

- [openSzigno](https://github.com/watt-mind/openSzigno) handles Microsec
  `.es3` dossiers. Its verification results concern the dossier it checked.
- [openPapir](https://github.com/watt-mind/openPapir) is the planned local
  correspondence workflow consumer: cases, imported packages, and receipts.

openKRX owns the container layer. It will preserve attachment bytes and
leave their interpretation to explicit downstream tools.

## Development

The workspace contains `openkrx-core` and `openkrx-cli`, both unpublished.
Pull requests target `develop`; `master` is reserved for stable releases.

```sh
bash scripts/check.sh
cargo build --release --locked
```

See [CONTRIBUTING.md](CONTRIBUTING.md), [SECURITY.md](SECURITY.md), and the
[documentation index](docs/index.md). The project is licensed under the
[MIT license](LICENSE) and is independent of government service operators.
