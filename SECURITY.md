# Security policy

## Current surface

The CLI supports help, version, capability reporting, the three reader
commands `inspect`, `list` and `validate-structure`, and `extract`. The
reader commands render the core library's bounded ZIP inventory
(`openkrx_core::archive::inventory`), bounded metadata parsing
(`openkrx_core::metadata::parse`) and structural check inventory
(`openkrx_core::profile::check`), all of which operate on caller-supplied
byte slices only. `extract` writes: it joins `openkrx_core::extract::plan`
onto a directory the caller names, under the no-clobber, no-link,
undo-on-failure policy mapped
[below](#threat-model-mapping-extraction-output-layer). The core crate itself
performs no I/O of any kind.

Reading the one input a command takes is bounded before parsing begins, and
writing happens only where `extract` was explicitly pointed. No command
creates a package: the library writes one — `openkrx_core::create::package`,
mapped [below](#threat-model-mapping-creation-layer) — but it returns bytes,
touches no filesystem, and no command exposes it. Nothing uses keys or
accesses government services. The requirements below
remain implementation gates for the package operations that do not exist
yet, not claims that a complete secure KRX parser exists. A successful
inventory, parse, structural report or extraction is an observation, never a
conformance or authenticity statement.

Only the current `develop` branch receives fixes. There are no released
versions, and no tags, to support yet; when releases begin, this section
will name the supported versions.

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
The extraction rows are in their own tables below: planning in
[one](#threat-model-mapping-extraction-planning-layer) and filesystem output
in [the other](#threat-model-mapping-extraction-output-layer). No package
*writer* exists, and those checks are unimplemented, not passing. The
no-panic row is held by the sweeps and, on random input, by the `inventory`
fuzz target and its bounded CI lane
([testing.md](docs/testing.md#fuzzing)).

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
| Diagnostics free of personal content and private paths | every code; `Display` prints code and limit numbers only. The `Debug` representation of a data type (`Metadata`, `StructureReport`, `Observations`, `AttachmentResolution`) prints package content verbatim and must never be logged, persisted or sent through telemetry. | `error_display_carries_codes_and_numbers_but_no_document_content` |
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
and consults no filesystem. **Every row here is planning only.** A row states
that an unsafe extraction is refused before it could start, never that a safe
extraction was performed; what happens at a real destination is the
[output layer](#threat-model-mapping-extraction-output-layer), and `extract`
surfaces every code below with its own exit status.

Test names are in `crates/openkrx-core/tests/`, except the two component
classes an accepted inventory can no longer carry, whose tests are the
`#[cfg(test)]` module in `crates/openkrx-core/src/extract/paths.rs`. Limits
appear in
[architecture.md](docs/architecture.md#extraction-planning-limits) and the
rules in
[architecture.md](docs/architecture.md#extraction-planning).

| Required check | Status | Error code prefix | Test |
| --- | --- | --- | --- |
| No traversal or absolute destination: every component is validated, and `..`, `.`, an empty component and a control character are refused | Planning; the output layer holds the rest | `extract.unsafe_path.parent_component`, `.current_component`, `.empty_component`, `.control_character` | `every_unsafe_component_class_reachable_from_an_archive_is_refused`, `every_unsafe_component_class_is_refused`, `a_name_mutation_sweep_never_panics_and_never_plans_an_unsafe_path` |
| No platform-ambiguous destination: a trailing dot or space, a leading space, a Windows reserved device name, a colon, and the six characters Windows refuses in a name (`*`, `?`, `<`, `>`, `\|`, `"`) are refused whatever the host platform | Planning; the output layer holds the rest | `extract.unsafe_path.trailing_dot`, `.trailing_space`, `.leading_space`, `.reserved_device_name`, `.colon`, `.reserved_character` | `every_unsafe_component_class_reachable_from_an_archive_is_refused`, `every_character_windows_refuses_outright_is_one_reserved_character_class` |
| No symlink or reparse-point escape: a link entry rejects the plan and is never created, nor written as its target text, on every host that carries a Unix mode (3 Unix and 19 OS X) | Planning; the output layer holds the rest | `extract.unsupported.link` | `a_symlink_entry_rejects_the_whole_plan`, `a_darwin_host_symlink_is_refused_like_a_unix_one`, `entry_kinds_are_read_from_the_central_directory_fields` |
| No special files: a device node, socket, FIFO or a Unix mode stating no file type rejects the plan | Planning; the output layer holds the rest | `extract.unsupported.special_file` | `a_special_file_entry_rejects_the_whole_plan` |
| No implicit character-set guessing for a destination name | Planning; the output layer holds the rest | `extract.unsupported.non_utf8_name` | `a_name_that_is_not_utf8_is_refused_rather_than_decoded` |
| Specified Unicode and case collisions, refused rather than resolved | Planning; the output layer holds the rest | `extract.ambiguous.collision`, `.file_directory_conflict` | `two_names_equal_after_normalisation_are_a_collision_not_a_choice`, `a_case_difference_that_only_appears_after_normalisation_is_a_collision`, `a_file_that_is_also_a_directory_prefix_is_refused`, `a_directory_prefix_that_arrives_before_its_file_is_refused_the_same_way`, `a_shared_file_name_in_different_directories_is_not_a_collision` |
| Documented output limits — file count, total bytes, path, component and depth — with checked arithmetic | Planning; the output layer holds the rest | `extract.over_limit.files`, `.total_bytes`, `.path_bytes`, `.component_bytes`, `.depth` | `the_file_count_limit_holds_at_its_boundary`, `the_total_size_limit_holds_at_its_boundary`, `the_path_length_limit_holds_at_its_boundary`, `the_component_length_limit_holds_at_its_boundary`, `the_depth_limit_holds_at_its_boundary` |
| A failure prevents the operation reporting success: a rejection rejects the whole plan, never a reduced one | Planning; the output layer holds the rest | every `extract.*` code | every rejection test above; each asserts that no plan was produced |
| No panic on any input | Planning; the output layer holds the rest | any code; never a panic | `planning_every_fixture_inventory_never_panics`, `a_name_mutation_sweep_never_panics_and_never_plans_an_unsafe_path` |
| Diagnostics free of personal content and private paths | Planning; the output layer holds the rest | every code; `Display` prints code, entry index and limit numbers only | `error_display_carries_codes_and_numbers_but_no_entry_name` |
| Confinement to a caller-selected destination, no-clobber creation, commit and cleanup after an interrupted write | Implemented, in the output layer | `output.*` | [the output table](#threat-model-mapping-extraction-output-layer) |

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
- **A host that carries a mode this reader does not map.** `EntryKind`
  reads `st_mode` for hosts 3 (Unix) and 19 (OS X), the two APPNOTE lists as
  storing it in the high attribute bits, so a link written on either is
  refused as a link. Should a further host be found to store a mode there,
  an entry it wrote would classify as `Unknown` and be planned as an
  ordinary file — its declared link target written as file content, never
  followed and never created as a link. The compensating control is that
  nothing writes yet, and that adding such a host is a one-line mapping with
  a test; no host is added on a guess.
- **A Unix mode of zero is treated as a special file.** Some writers set a
  Unix host system without a meaningful `st_mode`. Such an archive is
  refused rather than extracted on an assumption. If a real producer is ever
  observed doing this, it is a documented decision to revisit with evidence,
  not a bug to fix by guessing.
- **Planning proves nothing about writing.** A produced plan says a
  destination is describable, not that it can be created: permissions, an
  existing file, a case-insensitive filesystem and a concurrent writer are
  all invisible to a pure function. The filesystem ticket owns them.

## Threat-model mapping: extraction output layer

`crates/openkrx-cli/src/extract/` is the only code in openKRX that writes to
a filesystem. It adds no rule about names, entry kinds, collisions or
ceilings — those are the planner's, above — and owns the destination alone.
The policy, phase by phase, is in
[architecture.md](docs/architecture.md#extraction-output); the codes are
catalogued in [codes.md](docs/codes.md#output-codes). Test names in this
table are in `crates/openkrx-cli/tests/extract.rs`, except where noted.

Every failure exits 9 and leaves the destination as it was found: either the
run was refused before the first write, or the undo pass removed everything
this run had created. A partial result is never reported as a success.

| Required check | Error code prefix | Test |
| --- | --- | --- |
| Confinement to a caller-selected destination: it must already exist, be a real directory, and not be a symbolic link or reparse point; `extract` never creates it | `output.destination_missing`, `.destination_not_a_directory`, `.destination_symlink` | `a_destination_that_does_not_exist_is_refused_rather_than_created`, `a_destination_that_is_a_file_is_refused`, `a_destination_that_is_itself_a_symlink_is_refused` |
| No overwrite: a planned path that exists in any form — file, directory, symbolic link, or a link with a missing target — refuses the whole extraction before anything is written, as does an entry named like the marker this run creates | `output.exists` | `a_target_file_that_already_exists_refuses_the_whole_extraction`, `a_leaf_target_that_is_a_pre_existing_symlink_is_refused`, `an_entry_named_like_the_marker_is_refused_under_the_no_clobber_code` |
| No symlink or reparse-point escape through the path: every existing ancestor inside the destination must be a real directory, checked in preflight and again after each directory this run creates | `output.symlink_in_path`, `output.not_a_directory` | `an_ancestor_symlink_inside_the_destination_is_refused`, `a_destination_or_write_problem_is_nine` (`src/exit.rs`) |
| Exclusive creation: files with `create_new` (`O_EXCL` / `CREATE_NEW`), directories with `create_dir` and never `create_dir_all` | `output.exists`, `output.io` | `a_package_is_extracted_with_byte_identical_payloads`, `a_target_file_that_already_exists_refuses_the_whole_extraction` |
| Interrupted-write detection: a `.openkrx-extract.partial` marker exists for the length of the run, and its presence refuses the next one | `output.partial_marker_present` | `a_marker_left_by_an_interrupted_run_refuses_the_next_one`, `the_json_report_names_every_file_and_the_marker_it_removed` |
| Cleanup that never deletes pre-existing data: only paths this run created are removed, newest first, with `remove_dir` rather than `remove_dir_all` | `output.io`, and the `cleanup` counts | `a_failed_write_removes_this_runs_files_and_leaves_everything_else` |
| A failure never reports success, and never leaves partial output as a completed run | every `output.*` code, exit status 9 | every refusal test above; each reads the destination back |
| Payload bytes preserved exactly, with no mode bits, timestamps, links, special files or nested unpacking | not applicable: a preservation rule, not a refusal | `a_package_is_extracted_with_byte_identical_payloads`, `a_planner_refusal_keeps_its_own_category_and_names_no_path` |
| Diagnostics free of the destination path, an entry name and payload bytes; the written paths appear only in a successful report | every code; the diagnostic carries the code, its category and an entry index | `extract_never_carries_a_canary_in_a_diagnostic` (`tests/privacy.rs`), `a_planner_refusal_keeps_its_own_category_and_names_no_path` |
| No implicit execution or nested extraction | not applicable: the writer creates regular files and directories only, and never opens what it wrote | reviewed by construction |

### Residual risks of the output layer

- **A concurrent writer at the destination is out of scope.** The destination
  is trusted not to be modified by another principal while the command runs.
  `create_new` and the post-creation `symlink_metadata` checks defend against
  what is already there — an existing file, a symbolic link, a Windows
  reparse point — and not against an attacker who can act inside the window
  between a check and the operation that follows it. Closing that window
  needs `openat2` with `RESOLVE_BENEATH`, or the equivalent per-platform
  primitive, and is deliberately deferred. Extract into a directory only you
  can write to.
- **A crash leaves partial output.** A signal, a power loss or a killed
  process cannot run the undo pass, so partial files and the marker stay
  behind. The marker is the compensating control: a destination containing it
  is incomplete, the next run refuses it, and clearing it is a deliberate
  human act. Whole-tree atomicity through rename-based staging is deferred,
  with the reasons in
  [architecture.md](docs/architecture.md#cleanup-after-a-failed-write).
- **The undo pass can itself fail.** The condition that stopped the write —
  a read-only destination, a full disk — can stop the removal too. Such paths
  are counted as `left_in_place` rather than retried or hidden, so the report
  says the destination is not as it was found.
- **Windows junctions are not exercised by CI.** Reparse points are detected
  through `FILE_ATTRIBUTE_REPARSE_POINT`, but the runner cannot reliably
  create a junction or a symbolic link without developer mode or elevation,
  so the ancestor-link and leaf-link tests are `cfg(unix)`. The Windows half
  of the rule is held by construction and by the same code path, not by a
  test on that platform.
- **Extraction is not verification.** A file `extract` wrote is a byte-exact
  copy of what the package declared. It is not evidence that the package is
  authentic, that its contents are what they claim to be, or that a document
  inside it is safe to open. Attachments are opaque bytes and are never
  interpreted.

## Threat-model mapping: creation layer

`openkrx_core::create::package` writes the bytes of one package. It is a pure
function of the request: no filesystem, clock, process or network access, no
randomness, and no environment, so the same request always produces the same
bytes. **Nothing here writes a file.** Placing the bytes somewhere is the
command half of KRX-06 and does not exist; when it does, the no-clobber and
destination rules of the
[output layer](#threat-model-mapping-extraction-output-layer) apply to it.

The layer's premise is that a caller's request is as untrusted as an archive:
a file name, a description or a metadata value may come from anywhere, so
each is checked before it becomes part of a package, and a refusal refuses
the whole package. Tests are in
`crates/openkrx-core/tests/create_rejects.rs` and
`crates/openkrx-core/tests/create_package.rs`; the rules are in
[architecture.md](docs/architecture.md#deterministic-creation) and the codes
in [codes.md](docs/codes.md#creation-codes).

| Required check | Status | Error code prefix | Test |
| --- | --- | --- | --- |
| Documented limits enforced on the output, so a written package is readable under the same `Limits`: entry count, name length, per-entry and total decoded bytes, compression ratio, and the image length | Implemented | `create.over_limit.entries`, `.name_bytes`, `.entry_bytes`, `.total_bytes`, `.compression_ratio`, `.archive_bytes` | `the_entry_count_limit_holds_at_its_boundary`, `the_name_length_limit_holds_at_its_boundary`, `the_entry_size_limit_holds_at_its_boundary`, `the_total_size_limit_holds_at_its_boundary`, `the_archive_size_limit_holds_at_its_boundary`, `an_attachment_the_reader_would_call_a_bomb_is_refused`, `a_package_written_at_a_tightened_ceiling_reads_back_at_the_same_one`, `every_accepted_package_reads_back_entry_by_entry` |
| No package is written that the reader would refuse: what `package` returns passes `inventory`, the structural checks and `entry_bytes` on every entry under the same limits | Implemented | none; a property of the output, held by every ceiling above | `every_accepted_package_reads_back_entry_by_entry`, `a_written_package_fails_no_structural_check` |
| A record field is never silently truncated: a value a non-ZIP64 record cannot hold refuses the package | Implemented | `create.over_limit.archive_bytes`, `.entries` | `the_archive_size_limit_holds_at_its_boundary`, `the_entry_count_ceiling_is_what_the_end_record_can_count` |
| No unsafe entry name is ever written: the archive layer's name rules and the extraction planner's component rules both apply, so what openKRX writes it can read and extract on Linux, macOS and Windows alike | Implemented | `create.unsafe_name.*`, eleven classes | `every_unsafe_file_name_class_is_refused_with_its_own_code`, `a_name_the_extraction_planner_would_refuse_is_refused_at_creation` |
| A caller never builds a path: an attachment file name is one component, and the writer places it | Implemented | `create.unsafe_name.separator`, `.empty` | `every_unsafe_file_name_class_is_refused_with_its_own_code` |
| Attachment bytes are preserved exactly, never interpreted, converted or unpacked | Implemented | none; a property of the output | `every_attachment_reads_back_byte_identically`, `an_incompressible_attachment_still_reads_back_unchanged` |
| The document describes the package that was written: references and `MELLEKLETEK_SZAMA` are derived, and a disagreeing caller-supplied value is refused rather than written | Implemented | `create.invalid.reference_mismatch`, `.dispatch_count` | `a_supplied_reference_that_disagrees_with_the_attachments_is_refused`, `a_declared_count_that_disagrees_with_the_attachments_is_refused`, `attachments_with_nowhere_to_list_them_are_refused` |
| No XML injection and no silent rewriting: text is escaped deterministically, a character XML 1.0 cannot carry is refused, and text the reader would trim is refused rather than changed | Implemented | `create.invalid.text`, `.untrimmed_text` | `a_character_xml_cannot_carry_is_refused_rather_than_dropped`, `text_the_reader_would_trim_is_refused_rather_than_silently_changed`, `text_xml_can_carry_is_written_and_read_back_unchanged` |
| No content is dropped silently: a document carrying elements the grammar does not define is refused, because the writer cannot reproduce them | Implemented | `create.invalid.unknown_elements` | `a_document_carrying_elements_the_grammar_does_not_define_is_refused` |
| No clock, no environment and no ambient state: the modification time is the caller's, and identical requests produce identical bytes | Implemented | `create.invalid.timestamp` for a time the fields cannot express | `the_same_request_writes_the_same_bytes`, `the_timestamp_is_the_callers_and_reaches_every_record` |
| A failure prevents the operation reporting success: a refusal refuses the whole package, never a reduced one | Implemented | every `create.*` code | every rejection test above; each asserts that no bytes were produced |
| Diagnostics free of personal content and private paths | Implemented | every code; `Display` prints code, attachment index and limit numbers only | `every_unsafe_file_name_class_is_refused_with_its_own_code`, `a_declared_count_that_disagrees_with_the_attachments_is_refused` |
| No signing, encryption, `signatures.xml` or submission of any kind | By construction; openKRX performs no cryptography | none | — |

### Residual risks of this layer

- **Interoperability is unverified.** The writer emits the canonical
  documented layout. Interoperability with real producers is unverified
  because rules A19–A22 and M11–M15 remain unresolved; the reader's
  structural checks are the only gate, and a written package is
  "structurally consistent with the documented layout", never "conforming".
  No output of this project may say otherwise, and a package it writes still
  reports `Unresolved(A19)` and `Unresolved(M13)` when it reads it back.
- **The request is trusted for its content, not for its shape.** A caller
  decides what goes into a package; openKRX checks the shape — names, text,
  sizes, references — and never the meaning. A package can carry an
  attachment the caller should not have sent, and nothing here would know.
- **A payload can hold ZIP structure.** Attachment bytes are written
  verbatim, so a payload may contain byte sequences that look like ZIP
  records, including an end-of-central-directory signature. The reader
  refuses an image with two terminating candidates as an ambiguity rather
  than choosing one, so such a package is detected on the way back in rather
  than read two ways; a caller that must be certain reads its own output
  back, which `create::verify_round_trip` does in one call.

## Threat-model mapping: command-line input layer

The core crate performs no I/O, so reading the one input a reader command
takes is the only failure the command-line crate can produce on its own, and
the only place a private path could be disclosed. Test names in this table
are in `crates/openkrx-cli/tests/`. The command contract, the data shapes and
the eight exit statuses are in
[architecture.md](docs/architecture.md#command-contract-and-json-envelope) and
[Exit statuses](docs/architecture.md#exit-statuses); the two codes are
catalogued in [codes.md](docs/codes.md). The rows for what `extract` writes
are in [its own table](#threat-model-mapping-extraction-output-layer); no
package *writer* exists, and those checks are unimplemented, not passing.

| Required check | Error code prefix | Test |
| --- | --- | --- |
| Bounded input before parsing: at most `Limits::DEFAULT.max_archive_bytes + 1` bytes are buffered, and an input reaching that cap is refused before any parsing begins, from a file or from standard input alike | `input.over_limit.archive_bytes`, exit status 5 | `an_input_past_the_cap_is_refused_before_parsing`, `standard_input_past_the_cap_is_refused_too` |
| An input that cannot be opened or read is a content-free refusal, not a panic and not a path disclosure | `input.unreadable`, exit status 5 | `an_unreadable_input_is_status_five`, `a_directory_named_as_the_input_is_an_input_error_not_a_panic` |
| Diagnostics never carry the input path, an entry name or a metadata value: on standard error in every case, and on standard output whenever a run failed | every code; the diagnostic carries the code, its category, an entry index and numbers only | `standard_error_never_carries_a_canary_whatever_happened`, `a_failed_run_never_carries_a_canary_on_standard_output_either`, `the_input_path_is_absent_from_a_successful_report_as_well`, `a_declared_value_reaches_standard_output_only_for_inspect` |
| Attacker-controlled names and values reach a terminal escaped and bounded | not applicable: a rendering rule, not a refusal | `a_name_that_is_not_utf8_is_reported_as_bytes_rather_than_guessed`, `an_invisible_character_in_a_name_never_reaches_the_terminal`, `a_long_name_is_cut_and_the_remainder_is_counted` |
| A parsing failure keeps its category: the input cap is an input problem, and a package problem stays distinguishable from an unsupported feature and from a resource limit | `archive.*`, `metadata.*`, exit statuses 6, 7 and 8 | `a_malformed_image_is_status_six`, `an_unsupported_feature_is_status_seven`, `an_over_limit_archive_is_status_eight_with_its_numbers`, `every_catalogued_code_classifies_to_a_category` |
| No implicit execution, nested extraction, or remote access | not applicable: the executable opens the path it was given, reads it, and writes only where `extract` was pointed | reviewed by construction; nothing is logged, cached or persisted, and nothing written is ever opened again |

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
allowlisting is prohibited. Local private-corpus checks may report only
counts and stable error-code buckets: the one that exists,
`scripts/private-corpus.py`, is opt-in, is run by no test and no CI job, and
prints class counts, check outcomes and stable codes alone — never a file
name, entry name, path, metadata value, hash, timestamp or identifier. The
rules that bind it, and the maintainer running it, are in
[docs/roadmap.md](docs/roadmap.md#private-corpus-policy-for-maintainers).

## Interpretation boundary

Successful parsing, CRC checking, structural validation, or creation is not
cryptographic authentication, proof of delivery, or a legal determination.
An attachment verifier's result applies only to the identified attachment
and checks performed. openKRX must never promote that result to a verdict
on the enclosing package or other payloads.
