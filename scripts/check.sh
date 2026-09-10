#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
cargo machete
cargo deny --all-features check
python3 scripts/check-doc-links.py
python3 scripts/check-codes.py
# The envelope schema check needs the `jsonschema` package, which is a
# repository tool rather than a dependency of either crate. CI installs it and
# runs the check as a required step; here it is skipped loudly rather than
# failing a machine that does not have it.
if python3 -c "import jsonschema" >/dev/null 2>&1; then
  python3 scripts/check-schema.py
else
  echo "check-schema: skipped, the jsonschema package is not importable here;" \
    "install it with 'pip install jsonschema==4.23.0' to validate the JSON" \
    "envelopes against docs/schema/openkrx-envelope.v1.schema.json"
fi
python3 scripts/check-file-length.py
npx --yes markdownlint-cli2@0.18.1 "**/*.md" "#target" "#samples" "#refs" "#tmp" "#node_modules"
actionlint
