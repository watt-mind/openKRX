# Rule-to-implementation map

[profile.md](profile.md) numbers every rule the primary sources state:
A1 to A22 for the archive, M1 to M15 for the metadata document. This
document says, for each of them, what the code on the default branch
actually does: which module or structural check implements it, what outcome
the rule can produce, which test holds that behaviour, and a one-word status.

**This is not a conformance claim, and it cannot become one here.** Nine
rules are unresolved in the sources themselves, and [Known gaps](#known-gaps)
below names what would have to exist before any such claim could be made.

## How to read the status column

| Status | Meaning |
| --- | --- |
| **Asserted** | A check or a parser rule can conclude the rule holds or fails, and reports it. |
| **Observed** | The code reads and reports the fact, but never turns it into a pass or a failure. |
| **Unresolved** | The sources leave the rule open, so a check reports `Unresolved(rule)`, which is neither a pass nor a failure. |
| **Not asserted** | Deliberately not checked, with the reason given in the row. The rule is not implemented and never will be under the current evidence. |
| **Not implemented** | Nothing in the code addresses the rule yet. It is a candidate for later work, not a decision against it. |

Outcome semantics come from `CheckOutcome`: `Pass`, `Fail(code)`,
`Unresolved(rule)` and `NotApplicable`. Codes are catalogued in
[codes.md](codes.md); the eleven checks, in order, are in
[architecture.md](architecture.md#structural-check-inventory). Test names are
functions under `crates/openkrx-core/tests/`.

## Archive rules

| Rule | Implemented by | Outcome semantics | Test | Status |
| --- | --- | --- | --- | --- |
| A1: ZIP archive with the `.krx` extension | `archive::inventory` for the ZIP half; nothing for the extension, because the core crate takes byte slices and never sees a file name | An image that is not a ZIP archive fails the inventory with `archive.malformed.eocd_missing`; it is an error, not a check outcome | `missing_end_record_is_reported_as_missing` | Asserted (ZIP half only) |
| A2: first entry `mimetype` holding `application/OCD+ZIP` | Check 4 `marker_entry`, via `profile::locate::marker` | `Pass`; `Fail(metadata.malformed.marker_missing)`, `.marker_not_first` or `.marker_content`; `Unresolved(A19)` when the marker sits under a directory prefix | `an_archive_without_a_marker_fails_the_marker_check`, `a_marker_that_is_not_the_first_entry_fails_the_marker_check`, `a_marker_holding_something_else_fails_the_marker_check` | Asserted |
| A3: check the marker, then verify each listed attachment exists | Checks 4 and 8 together (`marker_entry`, `attachment_references`) | The two checks are reported separately and never combined into a verdict | `the_canonical_layout_passes_every_check_it_can_decide` | Asserted |
| A4: metadata lives in `Metalayer`, one `KULDEMENY_META.xml` | Check 1 `metadata_location`, via `profile::locate::locate` | `Pass`; `Fail(metadata.missing)` when nothing has the shape; `Fail(metadata.ambiguous.multiple_candidates)` when more than one does | `an_archive_without_a_metadata_document_fails_the_location_check`, `two_metadata_candidates_are_an_ambiguity_not_a_choice` | Asserted |
| A5: attachments under `Payload/`, one per subdirectory | Nothing. Attachment locations are read from the metadata document (check 8), never inferred from the entry-name layout | No outcome | — | Not asserted: A22 leaves subdirectory naming open, so a layout rule would be an invention |
| A6: two attachments may share a file name in different subdirectories | Check 10 `attachment_uniqueness`, via `profile::references::has_duplicate` | `Pass`; `Fail(metadata.reference.duplicate)` when two references share an attachment number or a joined path. A shared file name under different directories is not a collision | `a_shared_file_name_in_different_payload_directories_is_accepted`, `two_references_sharing_a_joined_path_fail` | Asserted |
| A7: optional `signatures.xml` with per-attachment digests | Nothing. The entry is inventoried by name like any other | No outcome | — | Not implemented: openKRX performs no cryptography (see the cryptographic boundary in architecture.md) |
| A8: `/` separators per APPNOTE, no root-directory marker | `archive::names` for the separator half, and `extract::plan`, which splits a name on `/` alone and validates every component | An image using a backslash fails the inventory with `archive.unsafe_name.backslash`; an absolute name fails with `archive.unsafe_name.absolute_path`. The root-directory-marker half is not checked | `every_unsafe_name_class_is_rejected` | Asserted (separator half only) |
| A9: further descriptive information may be present and is ignored | `archive::inventory` by construction: an entry it does not recognise is inventoried and never opened | No outcome | `stored_and_deflated_entries_are_listed_in_central_directory_order` | Observed |
| A10: payload subdirectories `ID-1`…`ID-4`, at most 10 | Nothing; the limit informs `Limits::DEFAULT.max_entries` (256) but is not enforced as a rule | No outcome | — | Not asserted: a service rule that A22 leaves unsettled in shape |
| A11: hybrid delivery requires `DeliveryInstruction.xml` | Nothing | No outcome | — | Not implemented: a service rule, not a container rule |
| A12: `DeliveryInstruction.xml.asice`, and both forms present is a rejection | Nothing | No outcome | — | Not implemented: a service rule |
| A13: `Cheques.xml` in an `ID`-prefixed subdirectory | Nothing | No outcome | — | Not implemented: a service rule |
| A14: envelope images `logo1.<ext>` / `picture1.<ext>` in `Metalayer` | Nothing | No outcome | — | Not implemented: a service rule |
| A15: `ControlMessage.xml` plus a certificate in `Payload/ID_1` | Nothing | No outcome | — | Not implemented: a service rule |
| A16: `message.properties` addressing file in `Metalayer` | Nothing | No outcome | — | Not implemented: a service rule the source itself calls transitional |
| A17: `.krx` file name starts with the KÉR root partner identifier | Nothing. The core crate never sees a file name | No outcome | — | Not implemented: needs a filesystem layer that does not exist |
| A18: certificates returned place the PDF in `Payload/ID-1` | Nothing | No outcome | — | Not asserted: classified as an example, not a requirement |
| A19: directory nesting and the location of `mimetype` | Checks 3 `root_prefix` and 4 `marker_entry` | `Pass` when the observed prefix is exactly `KRX/OCD/` and the marker is unprefixed; otherwise `Unresolved(A19)`. The observed prefix is reported as a fact in `Observations::root_prefix` | `a_shorter_root_prefix_is_unresolved_rather_than_wrong`, `a_marker_under_a_directory_prefix_is_unresolved_rather_than_wrong` | Unresolved |
| A20: the marker entry's compression method, extra fields and byte-exactness | Nothing. `profile::locate::marker` reads the entry's decoded content and nothing else | No outcome; a deflated marker produces no finding of any kind | `a_deflated_marker_is_not_treated_as_a_finding` | Not asserted: no source states the rule |
| A21: entry-name character encoding and case sensitivity | `archive::names` reports the consequence rather than the rule: names are compared as raw bytes, and a case-folded collision is an ambiguity. `extract::plan` adds the extraction-time consequence: a name that is not UTF-8 has no destination, and two paths equal after NFC and case folding are refused | Two names differing only by case fail the inventory with `archive.ambiguous.case_folded_duplicate_name`. The UTF-8 general-purpose flag and whether the bytes decode are reported independently, never merged. No check ever cites the rule itself | `duplicate_and_case_folded_names_are_ambiguities`, `utf8_flag_and_decodable_name_are_reported_independently` | Not asserted: no source states the rule, so only its consequence is reported |
| A22: payload subdirectory naming and numbering | Nothing. References are resolved against real entry names (check 8), never against a naming pattern | No outcome | — | Not asserted: the sources disagree, so any pattern would be an invention |

## Metadata rules

| Rule | Implemented by | Outcome semantics | Test | Status |
| --- | --- | --- | --- | --- |
| M1: target namespace, qualified form, root `KULDEMENY` | `metadata::reader`, reported through check 5 `metadata_parse` | `Fail(metadata.unsupported.namespace)` for another namespace or an unqualified root; `Fail(metadata.malformed.root_element)` for the wrong element in the right namespace. The two stay distinguishable on purpose | `a_root_in_another_namespace_is_unsupported_not_malformed`, `an_unqualified_root_is_unsupported_because_the_schema_is_qualified`, `a_wrong_root_element_in_the_right_namespace_is_malformed` | Asserted |
| M2: `KULDEMENY` child order, `FEJRESZ` then the optional blocks | `metadata::reader`, check 5 | `Fail(metadata.malformed.element_order)` out of order; `Fail(metadata.malformed.duplicate_element)` when a block repeats. This is the one rule that states an order, so order is enforced nowhere else | `sibling_blocks_out_of_the_documented_order_are_refused`, `a_repeated_sibling_block_is_refused` | Asserted |
| M3: `FEJRESZ` required and optional elements | `metadata::reader`, check 5 | `Fail(metadata.malformed.missing_element)` naming the field; `Fail(metadata.malformed.boolean)` for a non-boolean `TESZT`. An absent `TESZT` is recorded rather than defaulted, because M11 shows an official example omitting it | `a_missing_required_header_element_is_reported_with_its_field`, `a_non_boolean_teszt_is_refused`, `an_absent_teszt_is_recorded_rather_than_defaulted_silently` | Asserted |
| M4: `FORRASRENDSZER_AZONOSITO` and `KULDEMENY_TIPUS` enumerations | `metadata::reader`, check 5; both values are also reported in `Observations` | `Fail(metadata.malformed.enumeration)` outside the enumeration | `every_enumeration_token_the_schema_lists_is_accepted`, `a_value_outside_its_enumeration_is_refused` | Asserted |
| M5: `MELLEKLET` required and optional elements | `metadata::reader`, check 5; the reference itself is check 8 | `Fail(metadata.malformed.missing_element)`, `Fail(metadata.malformed.integer)` for a non-integer `CSATOLMANY_SZAMA`. An absent `MELLEKLET_LEIRASA` is recorded rather than rejected (M11), and a non-numeric `MERET` is kept as text | `a_non_integer_attachment_number_is_refused`, `an_absent_attachment_description_is_recorded_rather_than_rejected`, `a_non_numeric_declared_size_is_kept_as_text_rather_than_rejected` | Asserted (with the two M11 leniencies) |
| M6: `MERET` in kilobytes; `ELHELYEZKEDES` defaults to `KRX/OCD/Payload` | Partly. `MERET` is kept verbatim and never converted (see M13). The schema default is **not** substituted: an empty location leaves `FAJL_NEV` as the whole path, and `profile::references` tries the known root prefixes instead | No outcome of its own | `an_empty_location_leaves_the_file_name_as_the_whole_path` | Observed |
| M7: `MELLEKLETEK_SZAMA` alongside the list, both to be checked | Check 9 `attachment_count`, via `profile::references::count_agrees`. The reader also retains whether the `MELLEKLETEK` container itself was present, because an empty one and none at all are different documents | `Pass`; `Fail(metadata.count_mismatch)` on disagreement; `NotApplicable` when no dispatch declares a count | `a_declared_count_disagreeing_with_the_list_fails`, `a_document_declaring_no_count_leaves_the_count_check_inapplicable`, `an_empty_attachment_container_is_distinguished_from_no_container_at_all` | Asserted |
| M8: `KEZELESI_UTASITASOK` is namespace-unqualified | Check 7 `handling_instructions_form` | `Pass` when an unqualified element is observed; `NotApplicable` when no dispatch carries one. A qualified element of the same name is an unknown element, not this one, and is counted rather than reported | `an_unqualified_handling_instruction_is_observed_as_such`, `a_qualified_handling_instruction_is_an_unknown_element_not_the_real_one` | Asserted |
| M9: XML declaration and the `ns2` prefix binding in official examples | `metadata::scanner` accepts both a prefixed and a default binding, and a document with no declaration at all | No outcome: an example is never a requirement | `a_prefixed_document_reports_its_header_verbatim`, `a_default_namespace_binding_parses_the_same_as_a_prefixed_one`, `the_accepted_sample_structure_parses_into_the_documented_shape` | Observed |
| M10: a reference is `ELHELYEZKEDES` joined to `FAJL_NEV` | `profile::references`, reported through check 8 `attachment_references` | `Pass` when every joined path names an entry byte-exactly; `Fail(metadata.reference.missing_entry)` when one names none | `an_attachment_reference_keeps_its_declared_size_text_and_joined_path`, `a_trailing_separator_on_the_location_does_not_double_the_join`, `a_reference_to_a_missing_entry_fails` | Asserted |
| M11: an official example that does not validate against the schema | Check 6 `schema_optional_fields` | `Pass` when `TESZT`, every `MELLEKLET_LEIRASA` and every numeric `MERET` are present; otherwise `Unresolved(M11)`. Never a failure: the parser accepts the document either way | `an_omitted_schema_required_element_is_unresolved_rather_than_wrong` | Unresolved |
| M12: metadata file-name casing | Check 2 `metadata_file_name`, via `profile::locate` | `Pass` for the canonical `Metalayer/KULDEMENY_META.xml` spelling; otherwise `Unresolved(M12)`. The observed name is reported in `Observations::metadata_entry_name` | `a_lower_case_metadata_file_name_is_unresolved_rather_than_wrong`, `a_lower_case_metalayer_directory_is_also_unresolved` | Unresolved |
| M13: the unit and rounding of `MERET` | Check 11 `declared_size` | Always `Unresolved(M13)` when the document lists any attachment, and `NotApplicable` otherwise. The declared text, its numeric reading and the observed decoded size are reported side by side and never compared | `the_canonical_layout_passes_every_check_it_can_decide`, `the_accepted_sample_structure_resolves_inside_the_documented_layout`, and `a_report_answers_for_a_check_it_never_recorded` for the `NotApplicable` half | Unresolved |
| M14: whether `ELHELYEZKEDES` is archive-root-relative | Check 8 `attachment_references`, via `profile::references::resolve_path` | `Unresolved(M14)` when a reference resolves only after swapping one plausible root prefix for another; never a pass and never a failure | `a_reference_that_only_resolves_after_a_prefix_swap_is_unresolved`, `an_unprefixed_reference_resolves_against_a_prefixed_archive_as_a_variant` | Unresolved |
| M15: which fields a receiving service actually requires | Nothing, and nothing can | No outcome. Every check is internal to the archive; none can predict a service | — | Not asserted: the source states the requirement exists but not what it is |

## Extraction

Extraction is not a rule of [profile.md](profile.md); it is a requirement of
[SECURITY.md](../SECURITY.md#required-threat-model-for-package-support). Both
halves are implemented: `openkrx_core::extract` decides what an extraction
would create and refuses everything that could not be created safely, and
`crates/openkrx-cli/src/extract/` writes it into a destination the caller
names. The full mappings, with the code prefix and test for each check, are
in [SECURITY.md](../SECURITY.md#threat-model-mapping-extraction-planning-layer)
and [SECURITY.md](../SECURITY.md#threat-model-mapping-extraction-output-layer).

| Requirement | Implemented by | Outcome semantics | Test | Status |
| --- | --- | --- | --- | --- |
| No traversal, absolute or platform-ambiguous destination path | `extract::paths`, through `extract::plan` | Each unsafe component class rejects the whole plan with its own `extract.unsafe_path.*` code | `every_unsafe_component_class_reachable_from_an_archive_is_refused`, `every_unsafe_component_class_is_refused` | Asserted |
| No symlink or reparse-point escape, and no special files | `archive::kind`, through `extract::plan` | `extract.unsupported.link` and `extract.unsupported.special_file`; a directory marker produces no item | `a_symlink_entry_rejects_the_whole_plan`, `a_special_file_entry_rejects_the_whole_plan` | Asserted |
| Specified Unicode and case collisions, refused rather than resolved | `extract::collisions` | `extract.ambiguous.collision` and `.file_directory_conflict`; a shared file name under different directories is not a collision | `two_names_equal_after_normalisation_are_a_collision_not_a_choice`, `a_file_that_is_also_a_directory_prefix_is_refused` | Asserted |
| Documented output limits with checked arithmetic | `extract::ExtractLimits` | Each ceiling rejects the plan with its own `extract.over_limit.*` code | `the_file_count_limit_holds_at_its_boundary`, `the_total_size_limit_holds_at_its_boundary` | Asserted |
| Confinement to a caller-selected destination, no overwrite, no link escape | `cli::extract::preflight`, through `extract` | `output.destination_*`, `output.exists`, `output.symlink_in_path`, `output.not_a_directory`; exit status 9, and nothing written | `a_target_file_that_already_exists_refuses_the_whole_extraction`, `an_ancestor_symlink_inside_the_destination_is_refused`, `a_destination_that_is_itself_a_symlink_is_refused` | Asserted |
| Interrupted-write detection and cleanup that never deletes pre-existing data | `cli::extract::writer`, `cli::extract::cleanup` | `output.partial_marker_present`; a failed run removes only what it created and reports the counts | `a_marker_left_by_an_interrupted_run_refuses_the_next_one`, `a_failed_write_removes_this_runs_files_and_leaves_everything_else` | Asserted; a crash still leaves the marker, by design |

## Creation

Creation is implemented in the library and exposed by one command:
`openkrx_core::create::package` writes the bytes of one package, and
`openkrx create --manifest <FILE> --out <FILE>` places them in a file that
must not already exist, reading the result back through the structural checks
before reporting success. The writer emits
the canonical documented layout. Interoperability with real producers is
unverified because rules A19–A22 and M11–M15 remain unresolved; the reader's
structural checks are the only gate, and a written package is "structurally
consistent with the documented layout", never "conforming". What is written
and what is fixed is in
[architecture.md](architecture.md#deterministic-creation).

The rows below say what the writer does with the rules it touches. None of
them is a conformance claim, and none of them moves a rule's status: a rule
this project cannot decide when reading is one it cannot decide when writing
either.

| Rule | What the writer does | Outcome semantics | Test | Status |
| --- | --- | --- | --- | --- |
| A2: first entry `mimetype` holding `application/OCD+ZIP` | Writes it first, stored, holding exactly those bytes | Reading one back reports check 4 as `Unresolved(A19)`, because the entry is written under the `KRX/OCD/` prefix | `the_marker_is_the_first_entry_stored_verbatim`, `exactly_two_rules_stay_unresolved_over_a_written_package` | Observed |
| A4: metadata in `Metalayer`, one `KULDEMENY_META.xml` | Writes exactly one, at `KRX/OCD/Metalayer/KULDEMENY_META.xml` | Reading one back reports checks 1 and 2 as `Pass` | `the_entries_are_the_documented_layout_in_a_fixed_order` | Observed |
| A5, A22: attachments under `Payload/`, one subdirectory each | Writes `KRX/OCD/Payload/ID-<n>/<file>`, `n` 1-based, in request order | No outcome. The `ID-<n>` spelling is one of the three A22 leaves open, and choosing it resolves nothing | `the_entries_are_the_documented_layout_in_a_fixed_order` | Unresolved (A22 unchanged) |
| A7: optional `signatures.xml` | Never writes one | No outcome; openKRX performs no cryptography | — | Not implemented |
| A8: `/` separators, no root-directory marker | Builds every name itself from `/`-joined components, and refuses a file name holding a separator | `create.unsafe_name.separator`, and the other `create.unsafe_name.*` classes for the shapes the reader or the extraction planner would refuse | `every_unsafe_file_name_class_is_refused_with_its_own_code` | Asserted |
| A19: directory nesting and the location of `mimetype` | Writes the `KRX/OCD/` prefix, marker included, and says so | Reading one back reports check 4 as `Unresolved(A19)` and check 3 as `Pass` | `exactly_two_rules_stay_unresolved_over_a_written_package` | Unresolved |
| A20: the marker's compression method and byte-exactness | Stores the marker, because that is the convention the container family follows | No outcome. Nothing states the rule, so writing stored asserts nothing | `the_marker_is_the_first_entry_stored_verbatim` | Not asserted |
| A21: entry-name encoding and case | Writes UTF-8 names with general-purpose bit 11 set on every entry | No outcome. The flag is a fact about what was written, not a reading of the rule | `every_entry_declares_a_utf8_name_and_a_regular_file` | Not asserted |
| M1 to M5, M8, M9: the document grammar, order and prefix | Serialises the typed model in the M2, M3 and M5 order, in the target namespace under the `ns2` prefix, with `KEZELESI_UTASITASOK` unqualified | A document the reader parses back to the value that was written; a character XML 1.0 cannot carry is `create.invalid.text` | `the_document_is_the_shape_the_examples_show`, `every_optional_element_survives_the_round_trip` | Asserted |
| M6, M13: `MERET` and `ELHELYEZKEDES` | Derives `ELHELYEZKEDES` from where the attachment went, and writes `MERET` as kilobytes rounded up, the unit M6 documents | Reading one back still reports check 11 as `Unresolved(M13)`: writing a unit does not settle which one a service expects | `the_derived_references_describe_where_each_attachment_went`, `exactly_two_rules_stay_unresolved_over_a_written_package` | Unresolved |
| M7: `MELLEKLETEK_SZAMA` beside the list | Derives both from the attachments, and refuses a caller-supplied value that disagrees. The `MELLEKLETEK` container is written as the document carried it, and added whenever there is a reference to list | `create.invalid.reference_mismatch`; reading one back reports check 9 as `Pass` | `a_declared_count_that_disagrees_with_the_attachments_is_refused`, `the_attachment_container_is_written_exactly_as_the_document_carried_it` | Asserted |
| M10: a reference is `ELHELYEZKEDES` joined to `FAJL_NEV` | Writes references that resolve byte-exactly against the entry names it wrote | Reading one back reports check 8 as `Pass` with `ReferenceResolution::Resolved` | `exactly_two_rules_stay_unresolved_over_a_written_package` | Asserted |
| M11: elements one official example omits | Writes `TESZT` and `MELLEKLET_LEIRASA` when the request carries them, and omits them when it does not | Reading one back reports check 6 as `Pass` or as `Unresolved(M11)`, exactly as for a package read from elsewhere | `an_absent_teszt_element_stays_absent`, `a_package_written_without_descriptions_reports_m11_instead` | Unresolved |
| M12: metadata file-name casing | Writes the canonical spelling | Reading one back reports check 2 as `Pass`; the other spelling is still unresolved for a package written elsewhere | `the_entries_are_the_documented_layout_in_a_fixed_order` | Unresolved |
| M15: what a receiving service requires | Nothing. No package openKRX writes may be described as acceptable to any service | No outcome | — | Unresolved |

The command half adds two rows of its own, and no rule status:

| Rule | What the command does | Outcome semantics | Test | Status |
| --- | --- | --- | --- | --- |
| A19, M13 read back over what was written | Runs `archive::inventory`, `metadata::parse` and `profile::check` over the bytes it just wrote, before reporting success | `validate-structure` over a created package exits 4 and never 3: nothing fails, `marker_entry` cites A19, and any declared size cites M13. A failing check is a defect in openKRX, reported as `create.internal.self_check_failed`, and the file is removed | `a_created_package_reads_back_as_unresolved_and_never_as_inconsistent` (`crates/openkrx-cli/tests/create.rs`) | Unresolved |
| M11: elements one official example omits | Passes `MELLEKLET_LEIRASA` through from the manifest, and writes `TESZT` always | An attachment described in the manifest keeps check 6 at `Pass`; omitting `description` reports `Unresolved(M11)`, exactly as for a package read from elsewhere | `a_manifest_with_no_attachment_writes_the_two_fixed_entries` (`crates/openkrx-cli/tests/create.rs`) | Unresolved |

## Repacking

Repacking is implemented in the library and exposed by one command:
`openkrx_core::repack::{plan, apply}` decides and performs an edit, and
`openkrx repack <FILE|-> --edits <FILE> --out <FILE>` places the result in a
file that must not already exist, reading it back through the structural checks
before reporting success. It resolves no rule and moves no status: it composes
the three reading layers onto the writer, so every row above applies to what it
writes exactly as it applies to what `create` writes.

The rows below are what repacking adds. All three are refusals, and that is the
point: openKRX edits a package it could have written itself, and refuses every
other one rather than relaying it out under rules nobody has settled.

| Rule | What repacking does | Outcome semantics | Test | Status |
| --- | --- | --- | --- | --- |
| A19, A22, M12: the layout and the spellings the sources leave open | Reads the input as the canonical documented layout, byte-exactly, and refuses anything else | `repack.unsupported.root_prefix`, `.metadata_name`, `.marker`, `.extra_entry`. A refused package is not damaged: every reading command still reads it | `a_metadata_document_under_another_root_prefix_is_refused`, `a_metadata_file_name_spelled_differently_is_refused` | Unresolved (unchanged) |
| A7, A9, A11–A16, M2, M8: content the reader observes but does not retain | Refuses rather than dropping it: a signature document, a service-specific document, an element outside the grammar, or a block whose presence alone was recorded | `repack.unsupported.extra_entry`, `.unknown_elements`, `.opaque_block` | `an_entry_the_documented_layout_does_not_place_is_refused`, `a_document_carrying_elements_outside_the_grammar_is_refused`, `a_block_the_reader_records_only_the_presence_of_is_refused` | Not implemented (unchanged) |
| M5, M6, M7, M13: the derived references and the count | Re-derives every one of them from the attachments the result carries, and refuses an input whose own references the writer would derive differently. Whether the dispatch carried a `MELLEKLETEK` container is preserved rather than re-derived | `repack.unsupported.attachment_reference`, `.attachment_entry`; a preserved attachment's bytes are byte-identical in the result | `a_reference_the_writer_would_derive_differently_is_refused`, `every_preserved_attachment_keeps_its_bytes_exactly`, `a_dispatch_carrying_no_attachment_container_keeps_none` | Unresolved (unchanged) |

The command half adds one row, and no rule status:

| Rule | What the command does | Outcome semantics | Test | Status |
| --- | --- | --- | --- | --- |
| A19, M13 read back over what was written | Runs the structural checks over the bytes it just wrote, before reporting success | `validate-structure` over a repacked package exits 4 and never 3. A failing check is a defect in openKRX, reported as `repack.internal.self_check_failed`, and the file is removed | `the_report_says_what_changed_and_what_was_preserved` (`crates/openkrx-cli/tests/repack.rs`) | Unresolved |

## Known gaps

Nine of the thirty-seven rules cannot be decided from the retrieved sources:
A19, A20, A21, A22, M11, M12, M13, M14 and M15. Five of them are reported
as `Unresolved` by a named check (M12, A19, M11, M14, M13); the other four
are not asserted at all. Any one of them alone blocks a conformance claim.

What is missing, and what each would unblock:

- **A statement of the container layout from the format owner.** Three
  primary sources describe three different arrangements of `mimetype`,
  `KRX/`, `OCD/`, `Metalayer/` and `Payload/`, and none supersedes the
  others. Until one does, A19 stays unresolved, A22 cannot be given a
  pattern, and check 3 can only report what it observed.
- **A public sample `.krx` archive.** None was found. Without one, no
  archive-level rule has ever been checked against a real producer's
  output, and the synthetic fixtures only prove that openKRX agrees with
  its own reading of the documents.
- **An independent reference implementation or conformance suite.** None
  was found, so a reader/writer round-trip would only prove that our own
  components agree with each other — which is exactly what the writer's own
  round-trip test proves, and all it proves.
- **The upstream SPOCS OCD specification.** The catalogue entry
  `MKR-2.27` cites returns HTTP 404, so the conflicts above cannot be
  resolved against the container family's own definition.
- **A statement of the metadata file-name casing (M12) and of the `MERET`
  unit and rounding (M13).** Both are contradicted between sources. M13 in
  particular means a declared size can never be compared to an observed
  one, which is why check 11 exists but concludes nothing.
- **A statement of what a receiving service requires beyond schema
  validity (M15).** Without it, an internally consistent package can still
  be refused, so `StructureSummary::Consistent` can never mean "will be
  accepted".

Until those exist, openKRX reports observations. It does not, and must not,
state that an archive is a conforming package, or that a package it wrote is
one, and no output field carries such a verdict. The evidence itself, with
citations, is in
[profile.md](profile.md#conformance-evidence) and
[references.md](references.md).
