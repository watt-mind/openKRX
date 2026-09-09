# Security policy

## Current surface

The CLI supports only help, version, and capability reporting. The core
library additionally implements a bounded ZIP inventory
(`openkrx_core::archive::inventory`), bounded metadata parsing
(`openkrx_core::metadata::parse`) and a structural check inventory
(`openkrx_core::profile::check`) over caller-supplied byte slices; no command
exposes them. Nothing extracts files, uses keys, or accesses government
services. The requirements below remain implementation gates for package
support, not claims that a complete secure KRX parser exists. A successful
inventory, parse or structural report is an observation, never a conformance
or authenticity statement.

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
Extraction, output and writer rows do not exist yet; those checks are
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

## Threat-model mapping: metadata layer

Each required XML check above, the stable error-code prefix it produces, and
the test that holds it. Test names are in `crates/openkrx-core/tests/`. Limits
appear in [architecture.md](docs/architecture.md#metadata-limits); the check
inventory appears in
[architecture.md](docs/architecture.md#structural-check-inventory).

| Required check | Error code prefix | Test |
| --- | --- | --- |
| No XML DTDs or entity declarations | `metadata.unsupported.dtd` | `a_doctype_declaration_is_refused_before_anything_is_declared`, `an_external_entity_shaped_doctype_is_refused_as_a_doctype` |
| No entity or external resolution; only the five XML predefines and numeric character references | `metadata.malformed.entity` | `an_undeclared_entity_reference_is_refused_in_text`, `an_undeclared_entity_reference_is_refused_in_an_attribute_value`, `a_character_reference_to_a_forbidden_code_point_is_refused`, `predefined_entities_and_character_references_are_resolved_in_text` |
| No processing instructions other than the XML declaration | `metadata.unsupported.processing_instruction` | `a_processing_instruction_other_than_the_declaration_is_refused` |
| No implicit character-set guessing | `metadata.unsupported.encoding`, `metadata.malformed.encoding` | `a_non_utf8_encoding_declaration_is_refused_rather_than_guessed`, `bytes_that_are_not_utf8_are_refused_as_an_encoding_problem` |
| Documented document-size, XML-depth, node-count, attribute-count and text limits, enforced while streaming | `metadata.over_limit.document_bytes`, `.depth`, `.elements`, `.attributes_per_element`, `.text_bytes` | `the_document_size_limit_holds_at_its_boundary`, `the_depth_limit_holds_at_its_boundary`, `the_element_count_limit_holds_at_its_boundary`, `the_attribute_count_limit_holds_at_its_boundary`, `the_text_limit_holds_at_its_boundary`, `a_deeply_nested_unknown_subtree_still_meets_the_depth_limit` |
| Checked arithmetic, no panic on any input | any code; never a panic | `no_truncation_of_a_valid_document_is_ever_accepted_or_panics`, `single_byte_mutations_never_panic`, `a_truncated_prefix_of_every_refused_document_also_never_panics`, `no_truncation_of_a_krx_shaped_archive_ever_panics` |
| Reject ambiguity rather than selecting a convenient duplicate | `metadata.ambiguous.multiple_candidates`, `metadata.malformed.duplicate_element`, `metadata.malformed.unbound_prefix`, `metadata.reference.duplicate` | `two_metadata_candidates_are_an_ambiguity_not_a_choice`, `a_repeated_header_element_is_refused`, `a_repeated_sibling_block_is_refused`, `an_unbound_namespace_prefix_is_refused`, `two_references_sharing_an_attachment_number_fail`, `two_references_sharing_a_joined_path_fail` |
| Exact namespace and grammar rules, with unknown, malformed and unsupported cases distinguishable | `metadata.unsupported.namespace`, `metadata.malformed.root_element`, `.missing_element`, `.element_order`, `.enumeration`, `.integer`, `.boolean`, `.unexpected_child`, `.attribute`, `.syntax` | `a_root_in_another_namespace_is_unsupported_not_malformed`, `an_unqualified_root_is_unsupported_because_the_schema_is_qualified`, `a_wrong_root_element_in_the_right_namespace_is_malformed`, `a_missing_required_header_element_is_reported_with_its_field`, `sibling_blocks_out_of_the_documented_order_are_refused`, `a_value_outside_its_enumeration_is_refused`, `a_non_integer_attachment_number_is_refused`, `a_non_boolean_teszt_is_refused`, `a_child_element_inside_a_leaf_field_is_refused`, `a_repeated_attribute_is_refused`, `an_unclosed_element_is_refused_as_a_syntax_problem` |
| Declared attachments exist, and their references do not collide | `metadata.reference.missing_entry`, `metadata.count_mismatch` | `a_reference_to_a_missing_entry_fails`, `a_declared_count_disagreeing_with_the_list_fails`, `a_shared_file_name_in_different_payload_directories_is_accepted` |
| Unresolved rules stay distinguishable from failures | no code; `Unresolved(rule)` | `a_shorter_root_prefix_is_unresolved_rather_than_wrong`, `a_lower_case_metadata_file_name_is_unresolved_rather_than_wrong`, `a_marker_under_a_directory_prefix_is_unresolved_rather_than_wrong`, `a_reference_that_only_resolves_after_a_prefix_swap_is_unresolved`, `an_omitted_schema_required_element_is_unresolved_rather_than_wrong` |
| Diagnostics free of personal content and private paths | every code; `Display` prints code and limit numbers only | `error_display_carries_codes_and_numbers_but_no_document_content` |
| No implicit execution, nested extraction, or remote access | not applicable: the core crate has no filesystem, clock, process or network access | reviewed by construction; the crate's only dependencies are `serde`, `miniz_oxide` and `quick-xml` |

Attachment bytes are never decoded by this layer. Only the marker entry and
the metadata document are read back through `ArchiveInventory::entry_bytes`;
attachments stay opaque and are checked by name alone.

Neither layer is fuzzed yet. The exhaustive truncation sweeps and the
single-byte mutation sweeps are the compensating checks until a fuzz target
lands; an `xml_metadata` target belongs in that work alongside the archive
one.

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
