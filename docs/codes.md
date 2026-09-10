# Stable code catalogue

Every failure openKRX produces carries a stable dotted code. This document
lists all of them: the category the code belongs to, what it means, the
numeric fields the error carries alongside it, and the test that asserts it.

The codes are part of the public contract. Renaming one, or changing which
condition produces it, is a breaking change and belongs in
[CHANGELOG.md](../CHANGELOG.md). Adding one is not, which is why every error
enum is `#[non_exhaustive]`; a consumer matches on the code string and must
treat an unknown code as a failure rather than as a success.

`scripts/check-codes.py` keeps this catalogue honest. It extracts every
`"archive.…"`, `"create.…"`, `"extract.…"`, `"input.…"`, `"manifest.…"`,
`"metadata.…"`, `"output.…"` and `"repack.…"` string literal from
`crates/*/src/**` and
fails when a code exists in the sources but not here, or here but not in the
sources. It runs
as part of `bash scripts/check.sh`.

Test names below are functions in `crates/openkrx-core/tests/`, except in the
input, output and manifest sections, whose tests are in
`crates/openkrx-cli/tests/`.
Where a code has several asserting tests, the most specific one is named.

Each code also classifies to one exit status, listed in
[architecture.md](architecture.md#exit-statuses): `input.*` to 5,
`*.truncated.*`, `*.malformed.*`, `*.ambiguous.*`, `archive.unsafe_name.*` and
`archive.no_such_entry` to 6, `*.unsupported.*` to 7, and `*.over_limit.*` to
8 — except `input.over_limit.archive_bytes`, which is an input problem because
nothing was parsed at all. A structural-check code is never an exit status of
its own: `validate-structure` reports 3 when any check failed. The `extract.*`
codes follow the same segment rule as the archive ones — `extract.unsafe_path.*`
and `extract.ambiguous.*` to 6, `extract.unsupported.*` to 7,
`extract.over_limit.*` to 8 — and every `output.*` code classifies to 9, the
status the three commands that write, `extract`, `create` and `repack`, alone
exit with.
The `create.*` codes follow it too: `create.over_limit.*` to 8, and
`create.invalid.*`, `create.unsafe_name.*` and
`create.internal.self_check_failed` to 6, the status for a package that
contradicts itself — here, a request describing one that would, or a written
package openKRX could not read back. Every `manifest.invalid.*` code
classifies to 6 for the same reason: a manifest that cannot become a package
is that same reading, one step earlier. The `repack.*` codes split along the
same line: `repack.unsupported.*` to 7, because the package carries a feature
this build cannot re-emit, and `repack.invalid.*` and
`repack.internal.self_check_failed` to 6.

## Reading a diagnostic

No diagnostic ever carries content. `Display` for `ArchiveError` prints the
code, the numeric limit values where the error is a limit, the offending
numeric value where the error is an unsupported feature, and the
central-directory index of the entry. `Display` for `MetadataError` prints
the code and its numeric limit values only; the schema-fixed element an
error concerns is available as a typed `MetadataError::field`, never
printed, and document text, attribute values and entity names are never
exposed at all.

The **fields** column names the numeric data the error variant carries.
`entry` is a central-directory index, `limit_value` the configured ceiling,
`observed` the value that reached it, and `value` an offending numeric such
as a compression-method identifier. Where a field is optional, it is present
only when the failure is scoped to a single entry.

## Input codes

`openkrx-cli`, defined in `crates/openkrx-cli/src/input.rs` and
`crates/openkrx-cli/src/exit.rs`. These are the only codes the command-line
crate defines: the core crate performs no I/O, so reading the one input a
reader command takes is the only failure it can produce on its own. Neither
code carries a path, and neither says which of the possible I/O failures
occurred, because that distinction is not part of the contract and can leak
the shape of a filesystem.

| Code | Meaning | Fields | Asserted by |
| --- | --- | --- | --- |
| `input.unreadable` | The named file, or standard input, could not be opened or read: it does not exist, it is a directory, it is not permitted, or the read failed part-way. For `repack`, which opens three kinds of file, `field` says which one: `/` is the `--edits` document, `/add/path` or `/replace/path` a local file an edit names, and no field at all the package given as the argument. | `field`, `attachment` | `an_unreadable_input_is_status_five`, `an_unreadable_input_says_which_of_the_three_files_it_was` |
| `input.over_limit.archive_bytes` | The input reached the input cap, one byte past `Limits::DEFAULT.max_archive_bytes`, and was refused before any parsing began. `limit` is the archive ceiling and `observed` the cap, because reading stops there and the real length is never learned. | `limit`, `observed` | `an_input_past_the_cap_is_refused_before_parsing` |

## Output codes

`openkrx-cli`, defined in `crates/openkrx-cli/src/exit.rs` and produced by
`crates/openkrx-cli/src/output.rs`, `crates/openkrx-cli/src/extract/`,
`crates/openkrx-cli/src/create.rs` and `crates/openkrx-cli/src/repack.rs`.
These are the failures of the three commands that write. Every one of them
classifies to exit status 9, and every one of
them means **nothing incomplete was left behind**: before the first write,
because the run was refused during preflight; after it, because the undo pass
removed every path this run had created. A failed run never removes anything
that was already in the destination.

No code carries a path. The destination is the caller's own argument and a
diagnostic that repeated it could not be logged safely; the entry index says
which planned file the failure concerns, wherever the failure belongs to one.

The four codes the writing commands share — `output.destination_missing`,
`output.destination_not_a_directory`, `output.destination_symlink` and
`output.exists` — carry a different sentence for each of them in human mode,
because they are fixed by different actions: telling a caller who ran
`create` to extract into an empty directory is advice about a command they
did not run. The code and the exit status are the same either way.

The **fields** column is `entry` alone: an output failure carries no limit
value, because none of these conditions is a ceiling. A `create` refusal
carries no entry index either — nothing has been written when it is decided.
Tests are in `crates/openkrx-cli/tests/extract.rs` and
`crates/openkrx-cli/tests/create.rs`, except the classifier rows, which are
in `crates/openkrx-cli/src/exit/tests.rs`.

| Code | Meaning | Fields | Asserted by |
| --- | --- | --- | --- |
| `output.destination_missing` | The `--into` directory, or the parent directory of the `--out` file, does not exist. Neither command creates its destination: a typo would otherwise produce a new tree instead of a refusal. | — | `a_destination_that_does_not_exist_is_refused_rather_than_created`, `an_output_directory_that_is_missing_or_not_a_directory_is_refused` |
| `output.destination_not_a_directory` | The `--into` argument, or the parent of the `--out` argument, names something that exists but is not a directory. | — | `a_destination_that_is_a_file_is_refused` |
| `output.destination_symlink` | The `--into` argument, or the parent of the `--out` argument, names a symbolic link or a Windows reparse point. Both commands write only into a real directory, so that the destination the caller sees is the destination that is written. | — | `a_destination_that_is_itself_a_symlink_is_refused`, `an_output_directory_that_is_a_symbolic_link_is_refused` |
| `output.partial_marker_present` | The destination already holds `.openkrx-extract.partial`, so an earlier run into it did not finish and its output may be incomplete. Refused rather than added to. | — | `a_marker_left_by_an_interrupted_run_refuses_the_next_one` |
| `output.exists` | The `--out` file exists, in any form — a dangling symbolic link included — or a path an extraction would create already exists, in any form — file, directory, symbolic link or a dangling link — or is the `.openkrx-extract.partial` marker this run creates first. Nothing is ever overwritten, and the whole extraction is refused before anything is written. | `entry` | `a_target_file_that_already_exists_refuses_the_whole_extraction`, `a_leaf_target_that_is_a_pre_existing_symlink_is_refused`, `an_entry_named_like_the_marker_is_refused_under_the_no_clobber_code`, `a_directory_named_like_the_marker_is_refused_under_the_no_clobber_code` |
| `output.symlink_in_path` | An existing directory inside the destination that the package would write through is a symbolic link or a reparse point, which could place output outside the destination. Checked in preflight and again after each directory this run creates. | `entry` | `an_ancestor_symlink_inside_the_destination_is_refused` |
| `output.not_a_directory` | A path inside the destination that the package needs as a directory exists as something else. | `entry` | `a_destination_or_write_problem_is_nine` |
| `output.io` | A create, write or remove failed. The underlying reason is deliberately not reported: an operating-system message can name a path, and the distinction is not part of the contract. | `entry`, when a file was being written | `a_failed_write_removes_this_runs_files_and_leaves_everything_else` |

## Archive codes

`ArchiveError`, defined in `crates/openkrx-core/src/error.rs`. Six
categories keep truncation, self-contradiction, ambiguity, unsupported
features, resource exhaustion and unsafe names distinguishable from one
another, plus one lookup code.

### `archive.truncated.*`

The image ended inside a structure. Fields: `entry`, when entry-scoped.

| Code | Meaning | Asserted by |
| --- | --- | --- |
| `archive.truncated.central_directory` | A central-directory record header, or one of its variable-length fields, is cut short. | `a_central_record_cut_short_is_reported_as_truncation` |
| `archive.truncated.local_header` | A local file header, or one of its variable-length fields, is cut short. | `a_record_pointing_past_the_image_is_reported_as_truncation` |
| `archive.truncated.entry_data` | An entry declares more compressed data than the image holds. | `an_entry_declaring_more_data_than_the_image_holds_is_truncated` |
| `archive.truncated.data_descriptor` | A data descriptor runs into the end of the image. | `a_descriptor_running_into_the_end_of_the_image_is_truncated` |

### `archive.malformed.*`

A complete image that contradicts itself. Fields: `entry`, when
entry-scoped.

| Code | Meaning | Asserted by |
| --- | --- | --- |
| `archive.malformed.eocd_missing` | No end-of-central-directory record ends the image, so it is not a ZIP archive at all. | `missing_end_record_is_reported_as_missing` |
| `archive.malformed.record_signature` | A record did not carry the signature its position requires. | `a_record_without_its_signature_is_rejected` |
| `archive.malformed.central_directory_count` | The central directory holds a different record count than the end record declares. | `a_declared_record_count_above_the_real_one_is_rejected` |
| `archive.malformed.central_directory_size` | The central directory does not occupy exactly the declared byte range. | `a_declared_record_count_below_the_real_one_is_rejected` |
| `archive.malformed.central_directory_placement` | The central directory does not end where the end record begins. | `a_displaced_central_directory_is_rejected` |
| `archive.malformed.local_header_mismatch` | A local header contradicts its authoritative central-directory record on name, flags, method, CRC or a size. | `a_local_header_contradicting_its_record_is_rejected` |
| `archive.malformed.data_descriptor_mismatch` | A data descriptor contradicts its central-directory record, or appears without general-purpose bit 3. | `a_data_descriptor_contradicting_its_record_is_rejected` |
| `archive.malformed.overlapping_ranges` | Two claimed byte ranges overlap, so one byte belongs to two structures. | `overlapping_entry_ranges_are_rejected` |
| `archive.malformed.prefix_bytes` | Bytes precede the first local header. | `bytes_before_the_first_local_header_are_rejected` |
| `archive.malformed.trailing_bytes` | Bytes follow the end-of-central-directory comment. | `bytes_after_the_end_record_comment_are_rejected` |
| `archive.malformed.unclaimed_bytes` | Bytes belong to no local header, entry, descriptor, central-directory record or end record. | `bytes_claimed_by_no_structure_are_rejected` |
| `archive.malformed.crc_mismatch` | Decoded data did not match the declared CRC-32. This detects corruption, never authenticity. | `a_crc_that_does_not_match_the_decoded_bytes_is_rejected` |
| `archive.malformed.declared_size_mismatch` | The decoded size did not match the declared uncompressed size. | `a_declared_size_below_the_decoded_size_is_rejected` |
| `archive.malformed.deflate_stream` | The deflate stream is invalid, ends early, or carries bytes after its final block. | `a_corrupt_deflate_stream_is_rejected` |

### `archive.ambiguous.*`

Input that admits more than one reading; refused rather than resolved.
Fields: `entry`, when entry-scoped.

| Code | Meaning | Asserted by |
| --- | --- | --- |
| `archive.ambiguous.eocd` | More than one end-of-central-directory record terminates the image, so two readers could read two archives. | `a_second_end_record_candidate_is_an_ambiguity_not_a_choice` |
| `archive.ambiguous.duplicate_name` | Two entries carry byte-identical names. | `duplicate_and_case_folded_names_are_ambiguities` |
| `archive.ambiguous.case_folded_duplicate_name` | Two entry names differ only by case, and rule A21 leaves case rules unresolved, so neither reading can be preferred. | `duplicate_and_case_folded_names_are_ambiguities` |

### `archive.unsupported.*`

A well-formed ZIP feature this reader deliberately does not implement.
Fields: `value`, when the feature has an identifying number, and `entry`,
when entry-scoped.

| Code | Meaning | Asserted by |
| --- | --- | --- |
| `archive.unsupported.method` | A compression method other than stored (0) or deflate (8). | `unsupported_compression_methods_are_rejected` |
| `archive.unsupported.zip64` | A ZIP64 record, marker value, locator or extra field. | `zip64_records_and_markers_are_rejected` |
| `archive.unsupported.encryption` | An encryption or strong-encryption general-purpose flag. | `encryption_and_patched_data_flags_are_rejected` |
| `archive.unsupported.multi_disk` | A multi-disk or split archive, or a record on another disk. | `a_record_on_another_disk_is_rejected` |
| `archive.unsupported.patched_data` | Compressed patched data (general-purpose bit 5). | `encryption_and_patched_data_flags_are_rejected` |

### `archive.over_limit.*`

A documented ceiling was reached. Fields: `limit_value`, `observed` where
meaningful, and `entry` when entry-scoped. The defaults are in
[architecture.md](architecture.md#archive-inventory-limits).

| Code | Meaning | Asserted by |
| --- | --- | --- |
| `archive.over_limit.archive_bytes` | The input slice is longer than `max_archive_bytes`. | `archive_size_limit_holds_at_its_boundary` |
| `archive.over_limit.entries` | The end record declares more records than `max_entries`, checked before the directory is walked. | `entry_count_limit_holds_at_its_boundary` |
| `archive.over_limit.name_bytes` | An entry name is longer than `max_name_bytes`. | `name_length_limit_holds_at_its_boundary` |
| `archive.over_limit.entry_decoded_bytes` | One entry decoded more bytes than `max_entry_decoded_bytes`, counted while decoding. | `per_entry_decoded_size_limit_holds_at_its_boundary` |
| `archive.over_limit.total_decoded_bytes` | All entries together decoded more than `max_total_decoded_bytes`. | `total_decoded_size_limit_holds_at_its_boundary` |
| `archive.over_limit.compression_ratio` | An entry past the 64 KiB grace window exceeded `max_compression_ratio` decoded-to-compressed. | `a_deflate_bomb_is_stopped_by_the_ratio_limit` |
| `archive.over_limit.extra_field_bytes` | A local or central extra-field block is larger than `max_extra_field_bytes`. | `extra_field_limit_holds_at_its_boundary_in_both_headers` |
| `archive.over_limit.comment_bytes` | The archive comment or an entry comment is larger than `max_comment_bytes`. | `comment_limit_holds_at_its_boundary_for_archive_and_entry` |

### `archive.unsafe_name.*`

A name shape that must never reach a filesystem layer. Names are checked as
raw bytes, before any encoding decision, and the name itself is never
reported. Fields: `entry`, always present.

| Code | Meaning | Asserted by |
| --- | --- | --- |
| `archive.unsafe_name.empty` | The name is empty. | `every_unsafe_name_class_is_rejected` |
| `archive.unsafe_name.control_byte` | The name holds a NUL or another C0 control byte. | `every_unsafe_name_class_is_rejected` |
| `archive.unsafe_name.backslash` | The name holds a backslash, which rule A8 does not permit as a separator. | `every_unsafe_name_class_is_rejected` |
| `archive.unsafe_name.absolute_path` | The name starts with `/`. | `every_unsafe_name_class_is_rejected` |
| `archive.unsafe_name.parent_component` | The name holds a `..` path component. | `every_unsafe_name_class_is_rejected` |
| `archive.unsafe_name.drive_prefix` | The name starts with a Windows drive prefix such as `c:`. | `every_unsafe_name_class_is_rejected` |
| `archive.unsafe_name.directory_with_data` | The name ends with a separator but the entry declares content. | `every_unsafe_name_class_is_rejected` |

### Lookup

| Code | Category | Meaning | Fields | Asserted by |
| --- | --- | --- | --- | --- |
| `archive.no_such_entry` | Lookup | `ArchiveInventory::entry_bytes` was asked for an index the inventory does not hold. | `entry` | `entry_bytes_redecodes_only_the_requested_entry` |

## Metadata codes

`MetadataError`, defined in `crates/openkrx-core/src/metadata/error.rs`.
Three categories keep refused XML features, grammar violations and resource
exhaustion distinguishable. No variant carries an entry index, because the
parser reads one document and never sees the archive.

### `metadata.unsupported.*`

An XML feature or a namespace this reader deliberately refuses to process.
Fields: none.

| Code | Meaning | Asserted by |
| --- | --- | --- |
| `metadata.unsupported.dtd` | A `<!DOCTYPE ...>` declaration. Refused before anything can be declared, so no entity is ever declared and no external identifier is ever seen. | `a_doctype_declaration_is_refused_before_anything_is_declared` |
| `metadata.unsupported.processing_instruction` | A processing instruction other than the XML declaration. | `a_processing_instruction_other_than_the_declaration_is_refused` |
| `metadata.unsupported.encoding` | A declared character encoding other than UTF-8, refused rather than guessed at. | `a_non_utf8_encoding_declaration_is_refused_rather_than_guessed` |
| `metadata.unsupported.namespace` | The root element is bound to a namespace the profile does not define, or is unqualified while the schema is qualified (M1). | `a_root_in_another_namespace_is_unsupported_not_malformed` |

### `metadata.malformed.*` (parser)

A document that is not well-formed, or does not match the M1 to M8 grammar.
Fields: none numeric; a typed `MetadataError::field` names the schema-fixed
element where one applies, and is never printed.

| Code | Meaning | Asserted by |
| --- | --- | --- |
| `metadata.malformed.syntax` | The byte stream is not well-formed XML. | `an_unclosed_element_is_refused_as_a_syntax_problem` |
| `metadata.malformed.encoding` | The byte stream is not valid UTF-8. | `bytes_that_are_not_utf8_are_refused_as_an_encoding_problem` |
| `metadata.malformed.entity` | A general entity reference other than the five XML predefines, or a character reference to a forbidden code point. | `an_undeclared_entity_reference_is_refused_in_text` |
| `metadata.malformed.unbound_prefix` | A namespace prefix was used without being bound. | `an_unbound_namespace_prefix_is_refused` |
| `metadata.malformed.attribute` | An attribute is malformed, or repeated on one element. | `a_repeated_attribute_is_refused` |
| `metadata.malformed.root_element` | The root element is in the target namespace but is not `KULDEMENY` (M1). | `a_wrong_root_element_in_the_right_namespace_is_malformed` |
| `metadata.malformed.missing_element` | A required element is absent (M3, M5, M7). | `a_missing_required_header_element_is_reported_with_its_field` |
| `metadata.malformed.duplicate_element` | An element appears more than once where the grammar allows one. | `a_repeated_header_element_is_refused` |
| `metadata.malformed.element_order` | The `KULDEMENY` children appear in an order M2 does not allow. | `sibling_blocks_out_of_the_documented_order_are_refused` |
| `metadata.malformed.unexpected_child` | A leaf element that must carry text carries child elements instead. | `a_child_element_inside_a_leaf_field_is_refused` |
| `metadata.malformed.enumeration` | A value is outside its schema enumeration (M4). | `a_value_outside_its_enumeration_is_refused` |
| `metadata.malformed.integer` | A value declared `xs:long` is not an integer (M5, M7). | `a_non_integer_attachment_number_is_refused` |
| `metadata.malformed.boolean` | A value declared `xs:boolean` is not a boolean (M3). | `a_non_boolean_teszt_is_refused` |

### `metadata.over_limit.*`

A documented metadata ceiling was reached, counted inside the XML event
loop against values actually produced. Fields: `limit_value` and
`observed`. The defaults are in
[architecture.md](architecture.md#metadata-limits).

| Code | Meaning | Asserted by |
| --- | --- | --- |
| `metadata.over_limit.document_bytes` | The document slice is longer than `max_document_bytes`, checked before parsing starts. | `the_document_size_limit_holds_at_its_boundary` |
| `metadata.over_limit.depth` | Element nesting exceeded `max_depth`, counting the root as depth 1. | `the_depth_limit_holds_at_its_boundary` |
| `metadata.over_limit.elements` | More element start events than `max_elements` were produced. | `the_element_count_limit_holds_at_its_boundary` |
| `metadata.over_limit.attributes_per_element` | One element carried more attributes than `max_attributes_per_element`. | `the_attribute_count_limit_holds_at_its_boundary` |
| `metadata.over_limit.text_bytes` | Character data across the document exceeded `max_text_bytes`. | `the_text_limit_holds_at_its_boundary` |

## Structural check codes

`openkrx_core::profile::codes`, defined in
`crates/openkrx-core/src/profile/mod.rs`. These are the codes a failing
structural check reports as `CheckOutcome::Fail(code)`. They share the
`metadata.` namespace with the parser codes above and never collide with
one, so a consumer can bucket every diagnostic this crate produces by its
dotted code alone. A `CheckOutcome` carries the code string only, so none of
them carries a numeric field. Tests are in `profile_structure.rs`.

| Code | Category | Meaning | Asserted by |
| --- | --- | --- | --- |
| `metadata.missing` | Location | No entry has the shape of a metadata document (A4). | `an_archive_without_a_metadata_document_fails_the_location_check` |
| `metadata.ambiguous.multiple_candidates` | Location | More than one entry does, and nothing settles which one is meant. | `two_metadata_candidates_are_an_ambiguity_not_a_choice` |
| `metadata.malformed.marker_missing` | Marker | No entry's last path segment is `mimetype` (A2). | `an_archive_without_a_marker_fails_the_marker_check` |
| `metadata.malformed.marker_not_first` | Marker | A `mimetype` entry exists but is not the archive's first entry (A2). | `a_marker_that_is_not_the_first_entry_fails_the_marker_check` |
| `metadata.malformed.marker_content` | Marker | The marker entry holds something other than `application/OCD+ZIP` (A2). | `a_marker_holding_something_else_fails_the_marker_check` |
| `metadata.reference.missing_entry` | Reference | A declared attachment names no entry, with or without a plausible root prefix (M5, M10). | `a_reference_to_a_missing_entry_fails` |
| `metadata.reference.duplicate` | Reference | Two references share an attachment number, or a joined declared path (A6). | `two_references_sharing_an_attachment_number_fail` |
| `metadata.count_mismatch` | Count | `MELLEKLETEK_SZAMA` disagrees with the number of listed references (M7). | `a_declared_count_disagreeing_with_the_list_fails` |

## Extraction planning codes

`PlanError`, defined in `crates/openkrx-core/src/extract/error.rs`. These are
the codes `openkrx_core::extract::plan` produces when an inventory cannot be
turned into an extraction plan. Every one of them rejects the **whole** plan:
nothing is skipped, renamed or partially planned, because a caller handed a
quietly reduced plan would extract a package that is not the package it was
given. Fields: `entry`, always present except on the two whole-plan limits,
plus `limit_value` and `observed` on a limit.

No code here describes writing, because nothing writes: planning is a pure
function of the inventory and touches no filesystem. The filesystem half of
KRX-05 — a destination, no-clobber creation, cleanup after an interrupted
write — is a later ticket and will add codes of its own.

Tests are in `crates/openkrx-core/tests/extract_rejects.rs`, except the class
an accepted inventory can no longer carry — a `..` component, which
`archive::names` refuses first — whose tests are the `#[cfg(test)]` module in
`crates/openkrx-core/src/extract/paths.rs`.

### `extract.unsupported.*`

An entry this crate refuses to plan output for.

| Code | Meaning | Asserted by |
| --- | --- | --- |
| `extract.unsupported.link` | The entry declares a symbolic link (Unix `S_IFLNK`). No link is created, and its target text is not written as a file either. | `a_symlink_entry_rejects_the_whole_plan` |
| `extract.unsupported.special_file` | The entry declares a Unix mode that is neither a regular file, a directory nor a link: a device node, socket or FIFO, or a mode stating no file type at all. | `a_special_file_entry_rejects_the_whole_plan` |
| `extract.unsupported.non_utf8_name` | The entry name is not valid UTF-8. Rule A21 leaves the intended encoding unresolved, so no code page is guessed and no destination is invented. | `a_name_that_is_not_utf8_is_refused_rather_than_decoded` |

### `extract.unsafe_path.*`

A destination path component that must never reach a filesystem layer. The
component itself is never reported. The classes are checked in the order
below, so a component violating two of them always reports the first.

| Code | Meaning | Asserted by |
| --- | --- | --- |
| `extract.unsafe_path.empty_component` | A component is empty, as in `a//b`. | `every_unsafe_component_class_reachable_from_an_archive_is_refused` |
| `extract.unsafe_path.current_component` | A component is `.`. | `every_unsafe_component_class_reachable_from_an_archive_is_refused` |
| `extract.unsafe_path.parent_component` | A component is `..`. The archive layer already refuses such a name as `archive.unsafe_name.parent_component`, so this is defence in depth over the planner's own output. | `every_unsafe_component_class_is_refused`, `the_class_no_archive_can_carry_reports_its_stable_code` |
| `extract.unsafe_path.control_character` | A component holds a NUL, another C0 control, or a C1 control character. The C0 half is already refused by the archive layer; the C1 half is not, because those bytes are valid UTF-8 name bytes. | `every_unsafe_component_class_reachable_from_an_archive_is_refused` |
| `extract.unsafe_path.trailing_dot` | A component ends with `.`, which Windows silently strips, so two entries could become one file. | `every_unsafe_component_class_reachable_from_an_archive_is_refused` |
| `extract.unsafe_path.trailing_space` | A component ends with a space, stripped the same way. | `every_unsafe_component_class_reachable_from_an_archive_is_refused` |
| `extract.unsafe_path.leading_space` | A component starts with a space, which Windows Explorer and several tools strip the same way. | `every_unsafe_component_class_reachable_from_an_archive_is_refused` |
| `extract.unsafe_path.reserved_device_name` | A component is a Windows reserved device name — `CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9`, `LPT1`–`LPT9` — with or without an extension, compared ASCII case-insensitively. | `every_unsafe_component_class_reachable_from_an_archive_is_refused` |
| `extract.unsafe_path.colon` | A component holds `:`, which names an alternate data stream on NTFS. | `every_unsafe_component_class_reachable_from_an_archive_is_refused` |
| `extract.unsafe_path.reserved_character` | A component holds one of the six characters no Windows filesystem accepts in a name: `*`, `?`, `<`, `>`, `\|` or `"`. One code covers all six. A backslash is not among them: `archive.unsafe_name.backslash` refuses such a name before a plan is attempted. | `every_unsafe_component_class_reachable_from_an_archive_is_refused`, `every_character_windows_refuses_outright_is_one_reserved_character_class` |

### `extract.ambiguous.*`

Two planned outputs that cannot both exist; refused, never resolved by
renaming or skipping. The reported `entry` is the later of the two.

| Code | Meaning | Asserted by |
| --- | --- | --- |
| `extract.ambiguous.collision` | Two destination paths are equal after NFC normalisation and simple case folding, so a normalising or case-insensitive filesystem would see one path written twice. | `two_names_equal_after_normalisation_are_a_collision_not_a_choice`, `a_case_difference_that_only_appears_after_normalisation_is_a_collision` |
| `extract.ambiguous.file_directory_conflict` | One entry's destination path is a directory prefix of another's, so one name would have to be a file and a directory at once. The reported entry is the later of the two in either arrival order. | `a_file_that_is_also_a_directory_prefix_is_refused`, `a_directory_prefix_that_arrives_before_its_file_is_refused_the_same_way` |

### `extract.over_limit.*`

A documented extraction ceiling was exceeded. Fields: `limit_value`,
`observed` where meaningful, and `entry`. The defaults are in
[architecture.md](architecture.md#extraction-planning-limits).

| Code | Meaning | Asserted by |
| --- | --- | --- |
| `extract.over_limit.files` | More files would be created than `max_files`. Directory markers do not count: they produce no file. | `the_file_count_limit_holds_at_its_boundary` |
| `extract.over_limit.total_bytes` | The decoded bytes of all planned files together exceed `max_total_bytes`, summed with checked arithmetic. | `the_total_size_limit_holds_at_its_boundary` |
| `extract.over_limit.path_bytes` | One destination path is longer than `max_path_bytes`, separators included. | `the_path_length_limit_holds_at_its_boundary` |
| `extract.over_limit.component_bytes` | One path component is longer than `max_component_bytes`. | `the_component_length_limit_holds_at_its_boundary` |
| `extract.over_limit.depth` | A destination path has more components than `max_depth`. | `the_depth_limit_holds_at_its_boundary` |

## Creation codes

`CreateError`, defined in `crates/openkrx-core/src/create/error.rs`. These
are the codes `openkrx_core::create::package` produces when a request cannot
be written as the canonical documented layout. Every one of them refuses the
**whole** package: nothing is written partially, renamed or dropped. Fields:
`index`, the position of the attachment the failure concerns — a position in
the request, not a central-directory index, because nothing has been written
— plus `limit_value` and `observed` on a limit.

No code here describes a file, a path or a destination: the writer returns
bytes and touches no filesystem. Writing those bytes out is the command-line
half of KRX-06 and will add codes of its own.

Tests are in `crates/openkrx-core/tests/create_rejects.rs`, except the entry
count a non-ZIP64 end record cannot express, whose test is the `#[cfg(test)]`
module in `crates/openkrx-core/src/create/mod.rs`: writing 65 536 entries to
prove a `u16` would cost seconds of compression.

### `create.invalid.*`

A request that contradicts itself, or carries something the documented
layout cannot express.

| Code | Meaning | Asserted by |
| --- | --- | --- |
| `create.invalid.reference_mismatch` | A caller-supplied `MELLEKLET` reference, or `MELLEKLETEK_SZAMA`, disagrees with the attachments the request carries. The writer derives both from what it actually writes and never writes a document that describes a different package. | `a_supplied_reference_that_disagrees_with_the_attachments_is_refused`, `a_declared_count_that_disagrees_with_the_attachments_is_refused` |
| `create.invalid.dispatch_count` | Attachments were supplied with no `EXPEDIALAS` block to list them in, or the document carries more than one block, so there is no single place for the derived references. | `attachments_with_nowhere_to_list_them_are_refused`, `a_document_with_two_dispatch_blocks_is_refused` |
| `create.invalid.text` | A text value holds a character XML 1.0 cannot carry: a C0 control other than tab and line feed, a carriage return, which a parser would rewrite, or a non-character code point. | `a_character_xml_cannot_carry_is_refused_rather_than_dropped` |
| `create.invalid.untrimmed_text` | A text value starts or ends with whitespace. The reader trims leaf text, so the document would parse back to a different value than the one written. | `text_the_reader_would_trim_is_refused_rather_than_silently_changed` |
| `create.invalid.unknown_elements` | The document counted elements the grammar does not define (A9). The writer emits the grammar alone, so it refuses rather than dropping content silently. | `a_document_carrying_elements_the_grammar_does_not_define_is_refused` |
| `create.invalid.timestamp` | A calendar time falls outside what the MS-DOS date and time fields express: 1980-01-01 00:00:00 to 2107-12-31 23:59:59, with the month, day, hour, minute and second in range. | `a_timestamp_the_dos_fields_cannot_express_is_refused` |

### `create.over_limit.*`

A documented ceiling the output would exceed. The values are the reader's
own `Limits`, so a package this crate writes is one it reads back under the
same configuration. Fields: `limit_value`, `observed`, and `index` where the
attachment is known. The defaults are in
[architecture.md](architecture.md#archive-inventory-limits).

| Code | Meaning | Asserted by |
| --- | --- | --- |
| `create.over_limit.archive_bytes` | The assembled image would exceed `max_archive_bytes`, or the 4 GiB the non-ZIP64 records can address, whichever is smaller. The exact length is known before a byte is assembled, so no size or offset is ever truncated into a header. | `the_archive_size_limit_holds_at_its_boundary` |
| `create.over_limit.entries` | More entries than `max_entries`, counting the marker and the metadata document, or more than the 65 535 a non-ZIP64 end record can count, whichever is smaller. A relaxed `max_entries` refuses the package rather than truncating the count. | `the_entry_count_limit_holds_at_its_boundary`, `the_entry_count_ceiling_is_what_the_end_record_can_count` |
| `create.over_limit.name_bytes` | One entry name is longer than `max_name_bytes`. The two fixed names are checked on the same path as an attachment's. | `the_name_length_limit_holds_at_its_boundary` |
| `create.over_limit.entry_bytes` | One entry's bytes exceed `max_entry_decoded_bytes`, or the 4 GiB a non-ZIP64 size field holds. | `the_entry_size_limit_holds_at_its_boundary` |
| `create.over_limit.total_bytes` | Every entry together exceeds `max_total_decoded_bytes`. | `the_total_size_limit_holds_at_its_boundary` |
| `create.over_limit.compression_ratio` | One entry's decoded-to-stored ratio exceeds `max_compression_ratio`, once it has produced more than `Limits::RATIO_GRACE_BYTES` decoded bytes. It is the reader's own rule on the reader's own numbers: an entry the writer let through here is one `archive::inventory` would refuse while inflating it. | `an_attachment_the_reader_would_call_a_bomb_is_refused` |

### `create.unsafe_name.*`

An entry name this crate refuses to write, so that what openKRX writes it can
read back and extract on all three target platforms. The classes mirror
`archive.unsafe_name.*` and `extract.unsafe_path.*`; the name itself is never
reported. Every class below is asserted by
`every_unsafe_file_name_class_is_refused_with_its_own_code`.

| Code | Meaning |
| --- | --- |
| `create.unsafe_name.empty` | A file name, or a component of one, is empty. |
| `create.unsafe_name.separator` | A file name holds `/` or a backslash. An attachment file name is one component: the writer places it, and a caller never builds a path. |
| `create.unsafe_name.current_component` | A component is `.`. |
| `create.unsafe_name.parent_component` | A component is `..`. |
| `create.unsafe_name.control_character` | A component holds a C0 or C1 control character. |
| `create.unsafe_name.trailing_dot` | A component ends with `.`, which several filesystems silently strip. |
| `create.unsafe_name.trailing_space` | A component ends with a space, stripped the same way. |
| `create.unsafe_name.leading_space` | A component starts with a space, stripped the same way. |
| `create.unsafe_name.reserved_device_name` | A component is a Windows reserved device name, with or without an extension. |
| `create.unsafe_name.colon` | A component holds `:`, which names an alternate data stream on NTFS. |
| `create.unsafe_name.reserved_character` | A component holds one of `*`, `?`, `<`, `>`, `\|` or `"`. |

### `create.internal.*`

Defined in `crates/openkrx-cli/src/exit.rs` and produced by `create` alone.

| Code | Meaning | Fields | Asserted by |
| --- | --- | --- | --- |
| `create.internal.self_check_failed` | openKRX wrote a package and its own reader did not accept it: a structural check over the written bytes failed, or the bytes could not be read as an archive at all. This is a defect in openKRX, never in the manifest; the file is removed again and the run exits 6. | — | `the_self_check_code_is_a_package_problem` |

## Manifest codes

`openkrx-cli`, defined in `crates/openkrx-cli/src/exit.rs` and produced by
`crates/openkrx-cli/src/manifest.rs`. These are the failures of reading the
one JSON document `create` takes, and every one of them classifies to exit
status 6: a manifest that does not describe a package this build can write is
the same reading as a package that contradicts itself, one step earlier.
Nothing has been written when one is reported.

The **fields** column is `field` and `attachment`. `field` is a JSON Pointer
into the manifest — `/metadata/source_system`, `/attachments/path` — drawn
from the fixed set of schema paths in the manifest module, with array indices left
out; `attachment` is the position in the manifest's `attachments` array.
Neither is content: a value the manifest carries, and the spelling of a key
the schema does not define, are never reported. Tests are in
`crates/openkrx-cli/tests/create.rs`.

| Code | Meaning | Fields | Asserted by |
| --- | --- | --- | --- |
| `manifest.invalid.syntax` | The bytes are not one JSON object: not UTF-8, not parseable, an array or a scalar rather than an object, or carrying content after the object. | `field` | `each_manifest_defect_names_the_field_it_concerns` |
| `manifest.invalid.schema_version` | The manifest declares a `schema_version` this build does not implement. Version 1 is the only one it writes from. | `field` | `each_manifest_defect_names_the_field_it_concerns` |
| `manifest.invalid.unknown_field` | The manifest or the edits document, or an object inside it, carries a key the schema does not define. Refused rather than ignored: a misspelled `attachments` would otherwise write a package with no attachment and report success. `field` names the *object*, never the key, which is text the caller wrote; the human sentence lists the keys the *named object's* own schema defines instead, which is the actionable half and carries nothing of the input. | `field`, `attachment` | `an_unknown_key_is_refused_rather_than_ignored`, `a_defect_inside_an_attachment_names_its_position`, `the_unknown_key_sentence_lists_the_schemas_own_keys` |
| `manifest.invalid.missing_field` | A required field is absent or `null`. | `field`, `attachment` | `each_manifest_defect_names_the_field_it_concerns` |
| `manifest.invalid.type` | A field carries a JSON value of the wrong kind — a string where a boolean belongs, a scalar where an array does. | `field`, `attachment` | `each_manifest_defect_names_the_field_it_concerns` |
| `manifest.invalid.enumeration` | `source_system` or `consignment_kind` carries a token outside the fixed set rule M4 defines. The tokens are byte-exact. | `field` | `each_manifest_defect_names_the_field_it_concerns` |
| `manifest.invalid.timestamp` | `timestamp` is not of the form `YYYY-MM-DDTHH:MM:SS`, or names a time outside 1980-01-01T00:00:00 to 2107-12-31T23:59:58, which the MS-DOS fields of a ZIP record cannot express. | `field` | `each_manifest_defect_names_the_field_it_concerns` |

`repack` reads its edits document through the same module and reports the same
seven codes over it, with the same field paths for `metadata`, plus `/add`,
`/add/path`, `/add/file_name`, `/add/description`, `/replace`,
`/replace/number`, `/replace/path` and `/remove`. Its tests are in
`crates/openkrx-cli/tests/repack.rs`.

## Repacking codes

`RepackError`, defined in `crates/openkrx-core/src/repack/error.rs`, plus one
code the command-line crate defines. These are the failures of editing a
package that already exists.

**`repack.unsupported.*` is a statement about what openKRX can write, not
about the package.** A package refused here is not damaged and not malformed:
`inspect`, `list`, `validate-structure` and `extract` all still read it. The
writer emits one layout and one grammar, so anything the input carries that it
cannot re-emit — another root prefix, an entry the layout has no place for, an
element the reader counts but does not retain — is refused whole rather than
dropped from the result. The rule behind every row is one sentence: repacking
with no edit must produce the package it was given.

The **fields** column is `entry`, the central-directory index of the entry the
refusal concerns, and `number`, the attachment's `CSATOLMANY_SZAMA` in the
package being edited, counted from 1. Neither is content, and no edit a caller
wrote ever reaches a diagnostic. The three layers repacking composes report
through their own codes unchanged: `archive.*` for reading the package,
`metadata.*` for parsing its document and `create.*` for writing the result.

Tests are in `crates/openkrx-core/tests/repack_rejects.rs`, except
`repack.internal.self_check_failed`, whose test is in
`crates/openkrx-cli/src/exit/tests.rs`.

### `repack.unsupported.*`

| Code | Meaning | Fields | Asserted by |
| --- | --- | --- | --- |
| `repack.unsupported.metadata_missing` | No entry has the shape of a metadata document, or more than one does, so there is no single document to edit. | — | `an_archive_with_no_metadata_document_is_refused`, `an_archive_with_two_metadata_documents_is_refused` |
| `repack.unsupported.root_prefix` | The metadata document does not sit under `KRX/OCD/`. Rule A19 leaves the prefix open and the writer emits one of the three layouts, so repacking would move every entry of the package. | `entry` | `a_metadata_document_under_another_root_prefix_is_refused` |
| `repack.unsupported.metadata_name` | The document's last two segments are not `Metalayer/KULDEMENY_META.xml` byte-exactly. Rule M12 leaves the casing open, so re-emitting it would rename the entry. | `entry` | `a_metadata_file_name_spelled_differently_is_refused` |
| `repack.unsupported.marker` | The archive's first entry is not `KRX/OCD/mimetype` holding `application/OCD+ZIP` (A2, A19). | `entry` | `a_marker_that_is_missing_elsewhere_or_holds_something_else_is_refused` |
| `repack.unsupported.extra_entry` | An entry is neither the marker, the metadata document where the layout puts it, nor an attachment at `KRX/OCD/Payload/ID-<n>/<file>` numbered in entry order (A5, A22). A signature document (A7) and a service-specific document (A11–A16) reach this row: the writer emits neither, so repacking would drop them. | `entry` | `an_entry_the_documented_layout_does_not_place_is_refused` |
| `repack.unsupported.unknown_elements` | The document counted elements the grammar does not define (A9). The reader keeps none of them, so the writer cannot put them back. | — | `a_document_carrying_elements_outside_the_grammar_is_refused` |
| `repack.unsupported.opaque_block` | The document carries `ERKEZTETES`, `BONTASOK`, `TERTIVEVENY` (M2) or an unqualified `KEZELESI_UTASITASOK` (M8). The reader records their presence and never reads further, so re-emitting one would write it back empty. | — | `a_block_the_reader_records_only_the_presence_of_is_refused`, `an_unqualified_handling_instruction_element_is_refused` |
| `repack.unsupported.dispatch_count` | The document carries more than one `EXPEDIALAS` block, which leaves no single place for the derived references (M7). | — | `a_document_with_two_dispatch_blocks_is_refused` |
| `repack.unsupported.attachment_reference` | A `MELLEKLET` reference, or `MELLEKLETEK_SZAMA`, is not what this writer derives for the attachments the document declares — a different number, location, `MERET` value or count. Re-emitting the document would rewrite it. A dispatch that declares *no* count reaches this row too: the writer derives one and always emits it, so the document would gain the element (M7). | `entry`, `number` | `a_reference_the_writer_would_derive_differently_is_refused`, `a_declared_count_that_disagrees_with_the_references_is_refused`, `a_dispatch_that_declares_no_count_at_all_is_refused` |
| `repack.unsupported.attachment_entry` | The payload entries are not exactly the ones the references name, in the order the layout numbers them. | `entry`, `number` | `a_reference_naming_another_entry_is_refused`, `a_payload_entry_no_reference_declares_is_refused` |

### `repack.invalid.*`

An edit that does not name something the package holds. Nothing is written
when one is reported.

| Code | Meaning | Fields | Asserted by |
| --- | --- | --- | --- |
| `repack.invalid.no_such_attachment` | A `remove` or `replace` edit names an attachment number the package does not carry. Numbers are the document's own `CSATOLMANY_SZAMA`, counted from 1. | `number` | `an_edit_naming_an_attachment_the_package_does_not_hold_is_refused` |
| `repack.invalid.duplicate_target` | Two edits name the same attachment number, so what is to happen to it is not decided by the request. | `number` | `two_edits_naming_the_same_attachment_are_refused` |
| `repack.invalid.inventory_mismatch` | `repack::apply` was given an inventory other than the one `repack::plan` read — a different entry count, or an entry whose CRC-32 differs — so the bytes it would preserve are not the bytes the plan describes. A caller-side defect, and never reachable from the command line. | — | `applying_a_plan_to_another_package_is_refused_rather_than_written` |

### `repack.internal.*`

Defined in `crates/openkrx-cli/src/exit.rs` and produced by `repack` alone.

| Code | Meaning | Fields | Asserted by |
| --- | --- | --- | --- |
| `repack.internal.self_check_failed` | openKRX wrote a repacked package and its own reader did not accept it. This is a defect in openKRX, never in the edits; the file is removed again and the run exits 6. | — | `every_repacking_refusal_classifies_and_explains_itself` |

## What is deliberately absent

There is no success code, no `valid`, no `conforming` and no `is_krx`. A
check that cannot be decided reports `CheckOutcome::Unresolved(rule)`
naming the rule of [profile.md](profile.md) that blocks it, and that outcome
carries no code at all — it is not a failure and must never be rendered as
one. The unresolved rules and the checks that cite them are mapped in
[conformance.md](conformance.md).
