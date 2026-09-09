# Security policy

## Current surface

The CLI supports only help, version, and capability reporting. The core
library additionally implements a bounded ZIP inventory
(`openkrx_core::archive::inventory`) over a caller-supplied byte slice; no
command exposes it. Nothing extracts files, parses XML, uses keys, or accesses
government services. The requirements below remain implementation gates for
package support, not claims that a complete secure KRX parser exists. A
successful inventory is an observation, never a conformance or authenticity
statement.

Only the current `develop` branch receives fixes during scaffolding. There
are no released versions to support yet.

## Reporting

Use [GitHub private vulnerability reporting][report]. Do not publish a
vulnerability as a public issue. Include the affected commit, platform,
expected and actual behavior, and an independently authored synthetic
reproducer. Never send real correspondence or personal metadata.

[report]: https://github.com/watt-mind/openKRX/security/advisories/new

## Required threat model for package support

Assume an attacker controls every archive entry, byte, name, metadata field,
size declaration, and compression choice. Package support must establish:

- Fixed, documented input, entry-count, name-length, metadata, XML-depth,
  node-count, per-entry decoded-size, total decoded-size, and compression
  ratio limits, with checked arithmetic and streaming enforcement.
- No XML DTDs, entity declarations, external resolution, or implicit network
  access. Reject ambiguity rather than selecting a convenient duplicate.
- Detection of conflicting ZIP records, duplicate/colliding paths, unsafe
  names, unsupported methods, encryption, and truncated or corrupt streams.
  ZIP CRC checking concerns corruption, not authenticity.
- Extraction confined to a caller-selected destination, with no traversal,
  absolute paths, symlink/reparse-point escapes, special files, or overwrite.
  Specify Unicode/case collisions and platform-specific residual risks.
- A documented commit/cleanup policy for interrupted writes. Never delete
  pre-existing files or expose partial final output as a successful result.
- Bounded, terminal-safe output. Diagnostics must avoid personal content and
  private paths. Structured metadata is sensitive even when explicitly
  requested; never persist it through telemetry or automatic logging.
- No implicit execution, nested extraction, document rendering, signing,
  certificate verification, or remote submission.

Failure at a required check must prevent the operation from reporting
success. Resource limits must be enforced against actual decoded bytes,
not only attacker-supplied headers. Test these properties before enabling
package operations in the capabilities response.

## Threat-model mapping: archive layer

Each required archive check above, the stable error-code prefix it produces,
and the test that holds it. Test names are in `crates/openkrx-core/tests/`.
Limits appear in [architecture.md](docs/architecture.md#archive-inventory-limits).
XML, extraction, output and writer rows do not exist yet; those checks are
unimplemented, not passing.

| Required check | Error code prefix | Test |
| --- | --- | --- |
| Documented input, entry-count, name-length and extra/comment limits | `archive.over_limit.archive_bytes`, `.entries`, `.name_bytes`, `.extra_field_bytes`, `.comment_bytes` | `archive_size_limit_holds_at_its_boundary`, `entry_count_limit_holds_at_its_boundary`, `name_length_limit_holds_at_its_boundary`, `extra_field_limit_holds_at_its_boundary_in_both_headers`, `comment_limit_holds_at_its_boundary_for_archive_and_entry` |
| Per-entry and total decoded-size limits, enforced while decoding | `archive.over_limit.entry_decoded_bytes`, `.total_decoded_bytes` | `per_entry_decoded_size_limit_holds_at_its_boundary`, `total_decoded_size_limit_holds_at_its_boundary`, `a_deflate_bomb_is_also_stopped_by_the_per_entry_limit`, `a_deflate_bomb_is_also_stopped_by_the_total_limit` |
| Compression-ratio limit against actual decoded bytes | `archive.over_limit.compression_ratio` | `a_deflate_bomb_is_stopped_by_the_ratio_limit`, `a_small_entry_with_a_poor_ratio_stays_inside_the_grace_window` |
| Checked arithmetic, no panic on any input | any code; never a panic | `no_truncation_of_a_valid_archive_is_ever_accepted`, `single_byte_mutations_never_panic`, `stray_signatures_in_entry_data_never_panic` |
| Reject ambiguity rather than selecting a convenient duplicate | `archive.ambiguous.*` | `a_second_end_record_candidate_is_an_ambiguity_not_a_choice`, `duplicate_and_case_folded_names_are_ambiguities` |
| Detect conflicting ZIP records | `archive.malformed.local_header_mismatch`, `.data_descriptor_mismatch`, `.central_directory_count`, `.central_directory_size`, `.central_directory_placement`, `.record_signature`, `.overlapping_ranges`, `.prefix_bytes`, `.trailing_bytes`, `.unclaimed_bytes` | `a_local_header_contradicting_its_record_is_rejected`, `a_data_descriptor_contradicting_its_record_is_rejected`, `a_declared_record_count_above_the_real_one_is_rejected`, `a_declared_record_count_below_the_real_one_is_rejected`, `a_displaced_central_directory_is_rejected`, `a_record_without_its_signature_is_rejected`, `overlapping_entry_ranges_are_rejected`, `an_entry_range_reaching_into_the_central_directory_is_rejected`, `bytes_before_the_first_local_header_are_rejected`, `bytes_after_the_end_record_comment_are_rejected`, `bytes_claimed_by_no_structure_are_rejected` |
| Detect duplicate and colliding paths | `archive.ambiguous.duplicate_name`, `.case_folded_duplicate_name` | `duplicate_and_case_folded_names_are_ambiguities` |
| Detect unsafe names | `archive.unsafe_name.*` | `every_unsafe_name_class_is_rejected`, `names_that_merely_look_unusual_are_accepted` |
| Detect unsupported methods and encryption | `archive.unsupported.method`, `.encryption`, `.zip64`, `.multi_disk`, `.patched_data` | `unsupported_compression_methods_are_rejected`, `encryption_and_patched_data_flags_are_rejected`, `zip64_records_and_markers_are_rejected`, `a_record_on_another_disk_is_rejected`, `multi_disk_end_records_are_unsupported` |
| Detect truncated streams | `archive.truncated.*`, `archive.malformed.eocd_missing` | `no_truncation_of_a_valid_archive_is_ever_accepted`, `a_central_record_cut_short_is_reported_as_truncation`, `an_entry_declaring_more_data_than_the_image_holds_is_truncated`, `a_descriptor_running_into_the_end_of_the_image_is_truncated`, `missing_end_record_is_reported_as_missing` |
| Detect corrupt streams (corruption, not authenticity) | `archive.malformed.crc_mismatch`, `.declared_size_mismatch`, `.deflate_stream` | `a_crc_that_does_not_match_the_decoded_bytes_is_rejected`, `a_declared_size_below_the_decoded_size_is_rejected`, `a_declared_size_above_the_decoded_size_is_rejected`, `a_corrupt_deflate_stream_is_rejected`, `a_deflate_stream_that_ends_early_is_rejected` |
| Diagnostics free of personal content and private paths | every code; `Display` prints code, entry index and limit numbers only | `error_display_carries_codes_and_numbers_but_no_entry_name` |
| No implicit execution, nested extraction, or remote access | not applicable: the core crate has no filesystem, clock, process or network access | reviewed by construction; the crate's only dependencies are `serde` and `miniz_oxide` |

The archive layer is not fuzzed yet. The exhaustive truncation sweep and the
single-byte mutation sweep are the compensating checks until a fuzz target
lands.

## Data and key policy

Public fixtures are synthetic originals under their own
[licence](tests/fixtures/LICENSE). Do not commit private files, secrets,
`.env` files, private keys, or complete PEM private-key armour lines.
Do not derive public fixtures from private submissions. Secret-scanner
allowlisting is prohibited. Local private-corpus checks, if explicitly
introduced later, may report only counts and stable error-code buckets.

## Interpretation boundary

Successful parsing, CRC checking, structural validation, or creation is not
cryptographic authentication, proof of delivery, or a legal determination.
An attachment verifier's result applies only to the identified attachment
and checks performed. openKRX must never promote that result to a verdict
on the enclosing package or other payloads.
