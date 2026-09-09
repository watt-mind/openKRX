# Security policy

## Current surface

The CLI supports help, version, capability reporting, and the three reader
commands `inspect`, `list` and `validate-structure`. They render the core
library's bounded ZIP inventory (`openkrx_core::archive::inventory`), bounded
metadata parsing (`openkrx_core::metadata::parse`) and structural check
inventory (`openkrx_core::profile::check`), which still operate on
caller-supplied byte slices only. Reading the one input a command takes is
the whole of openKRX's I/O, and it is bounded before parsing begins. Nothing
extracts files, writes anywhere, uses keys, or accesses government
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
The extraction rows are in
[their own table below](#threat-model-mapping-extraction-planning-layer) and
are planning only; filesystem output and the writer rows do not exist yet,
and those checks are unimplemented, not passing. The no-panic row is held by
the sweeps and, on random input, by the `inventory` fuzz target and its
bounded CI lane ([testing.md](docs/testing.md#fuzzing)).

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
| Detect truncated streams | `archive.truncated.*`, catalogued in [docs/codes.md](docs/codes.md#archivetruncated) | `no_truncation_of_a_valid_archive_is_ever_accepted`, `a_central_record_cut_short_is_reported_as_truncation`, `an_entry_declaring_more_data_than_the_image_holds_is_truncated`, `a_descriptor_running_into_the_end_of_the_image_is_truncated` |
| Detect an image that is not a ZIP archive at all | `archive.malformed.eocd_missing` | `missing_end_record_is_reported_as_missing` |
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

Both layers are fuzzed. `inventory` drives `archive::inventory` and
`entry_bytes`, `xml_metadata` drives `metadata::parse`, and each runs for a
bounded 30 seconds per pull request; the exhaustive truncation and single-byte
mutation sweeps remain the load-bearing compensating checks, because a short
run is not a campaign. Both are described in
[testing.md](docs/testing.md#fuzzing), with what the lane still does not do.

## Threat-model mapping: extraction planning layer

`openkrx_core::extract::plan` decides what an extraction would create. It is
a pure function of the archive inventory: it opens nothing, writes nothing
and consults no filesystem. **Every row here is planning only; filesystem
output is pending.** A row states that an unsafe extraction is refused before
it could start, never that a safe extraction was performed, and no command
reaches this layer yet — `capabilities().operations` still names the three
reader commands only.

Test names are in `crates/openkrx-core/tests/`, except the two component
classes an accepted inventory can no longer carry, whose tests are the
`#[cfg(test)]` module in `crates/openkrx-core/src/extract/paths.rs`. Limits
appear in
[architecture.md](docs/architecture.md#extraction-planning-limits) and the
rules in
[architecture.md](docs/architecture.md#extraction-planning).

| Required check | Status | Error code prefix | Test |
| --- | --- | --- | --- |
| No traversal or absolute destination: every component is validated, and `..`, `.`, an empty component and a control character are refused | Planning only; filesystem output pending | `extract.unsafe_path.parent_component`, `.current_component`, `.empty_component`, `.control_character` | `every_unsafe_component_class_reachable_from_an_archive_is_refused`, `every_unsafe_component_class_is_refused`, `a_name_mutation_sweep_never_panics_and_never_plans_an_unsafe_path` |
| No platform-ambiguous destination: a trailing dot or space, a Windows reserved device name, and a colon are refused whatever the host platform | Planning only; filesystem output pending | `extract.unsafe_path.trailing_dot`, `.trailing_space`, `.reserved_device_name`, `.colon` | `every_unsafe_component_class_reachable_from_an_archive_is_refused` |
| No symlink or reparse-point escape: a link entry rejects the plan and is never created, nor written as its target text | Planning only; filesystem output pending | `extract.unsupported.link` | `a_symlink_entry_rejects_the_whole_plan`, `entry_kinds_are_read_from_the_central_directory_fields` |
| No special files: a device node, socket, FIFO or a Unix mode stating no file type rejects the plan | Planning only; filesystem output pending | `extract.unsupported.special_file` | `a_special_file_entry_rejects_the_whole_plan` |
| No implicit character-set guessing for a destination name | Planning only; filesystem output pending | `extract.unsupported.non_utf8_name` | `a_name_that_is_not_utf8_is_refused_rather_than_decoded` |
| Specified Unicode and case collisions, refused rather than resolved | Planning only; filesystem output pending | `extract.ambiguous.collision`, `.file_directory_conflict` | `two_names_equal_after_normalisation_are_a_collision_not_a_choice`, `a_case_difference_that_only_appears_after_normalisation_is_a_collision`, `a_file_that_is_also_a_directory_prefix_is_refused`, `a_shared_file_name_in_different_directories_is_not_a_collision` |
| Documented output limits — file count, total bytes, path, component and depth — with checked arithmetic | Planning only; filesystem output pending | `extract.over_limit.files`, `.total_bytes`, `.path_bytes`, `.component_bytes`, `.depth` | `the_file_count_limit_holds_at_its_boundary`, `the_total_size_limit_holds_at_its_boundary`, `the_path_length_limit_holds_at_its_boundary`, `the_component_length_limit_holds_at_its_boundary`, `the_depth_limit_holds_at_its_boundary` |
| A failure prevents the operation reporting success: a rejection rejects the whole plan, never a reduced one | Planning only; filesystem output pending | every `extract.*` code | every rejection test above; each asserts that no plan was produced |
| No panic on any input | Planning only; filesystem output pending | any code; never a panic | `planning_every_fixture_inventory_never_panics`, `a_name_mutation_sweep_never_panics_and_never_plans_an_unsafe_path` |
| Diagnostics free of personal content and private paths | Planning only; filesystem output pending | every code; `Display` prints code, entry index and limit numbers only | `error_display_carries_codes_and_numbers_but_no_entry_name` |
| Confinement to a caller-selected destination, no-clobber creation, commit and cleanup after an interrupted write | **Not implemented** | — | — |

### Residual risks of this layer

- **Case folding is the simple mapping.** Collision detection normalises to
  NFC and lower-cases with `char::to_lowercase`, the Unicode *simple*
  lowercase mapping, not full case folding. The two agree for the Latin,
  Greek and Cyrillic text these names can realistically hold, but a pair
  that only full case folding equates — a Cherokee or Deseret pair, or a
  final sigma against its non-final form in an unusual position — would pass
  planning and could still collide on a case-insensitive filesystem. A
  no-clobber creation rule in the filesystem half is the compensating
  control, and it is not implemented yet.
- **Normalisation form is a choice, not a fact.** A filesystem that stores
  NFD (older APFS behaviour) or normalises on lookup may equate paths this
  layer keeps apart in the other direction. The plan is refused where a
  collision is visible under NFC; nothing here can predict every
  filesystem's own folding.
- **A Unix mode of zero is treated as a special file.** Some writers set a
  Unix host system without a meaningful `st_mode`. Such an archive is
  refused rather than extracted on an assumption. If a real producer is ever
  observed doing this, it is a documented decision to revisit with evidence,
  not a bug to fix by guessing.
- **Planning proves nothing about writing.** A produced plan says a
  destination is describable, not that it can be created: permissions, an
  existing file, a case-insensitive filesystem and a concurrent writer are
  all invisible to a pure function. The filesystem ticket owns them.

## Threat-model mapping: command-line input layer

The core crate performs no I/O, so reading the one input a reader command
takes is the only failure the command-line crate can produce on its own, and
the only place a private path could be disclosed. Test names in this table
are in `crates/openkrx-cli/tests/`. The command contract, the data shapes and
the eight exit statuses are in
[architecture.md](docs/architecture.md#command-contract-and-json-envelope) and
[Exit statuses](docs/architecture.md#exit-statuses); the two codes are
catalogued in [codes.md](docs/codes.md). No extraction, output or writer row
exists yet; those checks are unimplemented, not passing.

| Required check | Error code prefix | Test |
| --- | --- | --- |
| Bounded input before parsing: at most `Limits::DEFAULT.max_archive_bytes + 1` bytes are buffered, and an input reaching that cap is refused before any parsing begins, from a file or from standard input alike | `input.over_limit.archive_bytes`, exit status 5 | `an_input_past_the_cap_is_refused_before_parsing`, `standard_input_past_the_cap_is_refused_too` |
| An input that cannot be opened or read is a content-free refusal, not a panic and not a path disclosure | `input.unreadable`, exit status 5 | `an_unreadable_input_is_status_five`, `a_directory_named_as_the_input_is_an_input_error_not_a_panic` |
| Diagnostics never carry the input path, an entry name or a metadata value: on standard error in every case, and on standard output whenever a run failed | every code; the diagnostic carries the code, its category, an entry index and numbers only | `standard_error_never_carries_a_canary_whatever_happened`, `a_failed_run_never_carries_a_canary_on_standard_output_either`, `the_input_path_is_absent_from_a_successful_report_as_well`, `a_declared_value_reaches_standard_output_only_for_inspect` |
| Attacker-controlled names and values reach a terminal escaped and bounded | not applicable: a rendering rule, not a refusal | `a_name_that_is_not_utf8_is_reported_as_bytes_rather_than_guessed`, `an_invisible_character_in_a_name_never_reaches_the_terminal`, `a_long_name_is_cut_and_the_remainder_is_counted` |
| A parsing failure keeps its category: the input cap is an input problem, and a package problem stays distinguishable from an unsupported feature and from a resource limit | `archive.*`, `metadata.*`, exit statuses 6, 7 and 8 | `a_malformed_image_is_status_six`, `an_unsupported_feature_is_status_seven`, `an_over_limit_archive_is_status_eight_with_its_numbers`, `every_catalogued_code_classifies_to_a_category` |
| No implicit execution, nested extraction, or remote access | not applicable: the executable opens the path it was given, reads it, and writes nothing anywhere | reviewed by construction; nothing is logged, cached or persisted |

The file argument is opened exactly as written, with no path normalisation,
globbing, symlink resolution or extension inference on any platform, and `-`
reads standard input as binary. Attachments are never decoded, and no
declared metadata value is printed except by `inspect`, on standard output,
because that is what it was asked for.

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
