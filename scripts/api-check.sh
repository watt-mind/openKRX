#!/usr/bin/env bash
# Informational public-API compatibility report for one crate.
#
# Usage: scripts/api-check.sh [base-rev] [crate]
#
# Compares the crate's public API in the working tree against the same crate at
# a git revision and prints what cargo-semver-checks found. It reports; it does
# not gate. Nothing is published: both crates keep `publish = false`, so the
# baseline is a commit rather than a registry release.
set -euo pipefail
cd "$(dirname "$0")/.."

base_rev="${1:-origin/develop}"
crate="${2:-openkrx-core}"

if ! command -v cargo-semver-checks > /dev/null 2>&1; then
  echo "cargo-semver-checks is not installed: cargo install cargo-semver-checks --locked" >&2
  exit 127
fi

if ! git rev-parse --verify --quiet "${base_rev}^{commit}" > /dev/null; then
  echo "base revision '${base_rev}' is not a commit in this repository" >&2
  echo "a shallow clone has no history to compare against: fetch it first" >&2
  exit 2
fi

base_sha="$(git rev-parse "${base_rev}")"
echo "Comparing ${crate} against ${base_rev} (${base_sha})"

# `--release-type minor` is load-bearing. The workspace version is
# 0.1.0-dev.0 on both sides of every comparison, and an unchanged version
# makes cargo-semver-checks assume a major bump, under which every
# breaking-change lint is allowed and therefore skipped -- a run that reports
# nothing whatever the diff did. Declaring the comparison a minor release
# runs those lints, which is the question worth asking: would this change
# break a downstream build if the API were already published?
exec cargo semver-checks check-release \
  --package "${crate}" \
  --baseline-rev "${base_sha}" \
  --release-type minor
