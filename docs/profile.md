# First supported KRX profile: evidence

Status checked: 2026-09-09. This document records what public primary
sources actually state about one KRX profile. It is research output, not a
conformance claim, and no parser or writer exists. Nothing here authorises
describing openKRX as compatible with any service. Unresolved rows below
block structural-validation and writer conformance claims outright.

## Selected target profile

| Property | Value |
| --- | --- |
| Producer/consumer | Magyar Posta Zrt. Hivatali kapu / hybrid delivery (KRID 326773742 for the electronic dispatch list gateway, 506341775 for the hybrid service) |
| Container family | KRX, the Hungarian realisation of the SPOCS OCD container |
| Container marker | `mimetype` entry with content `application/OCD+ZIP` |
| Metadata document | `KULDEMENY_META.xml` |
| Metadata namespace | `http://xsd.orfk.hu/rzs/ker/kuldemeny` |
| Metadata schema | `KER_META_V0_9.xsd`, maintained by ORFK |
| Profile version token | `KRX_VERZIOSZAM` = `v0.9` (enumeration `v0.1`…`v0.9`) |

The version token is the only machine-readable profile version found in any
primary source. The archive layout itself carries no version marker, so an
archive-level version cannot be detected without reading the metadata.

## Evidence classes

- **Normative** — stated as a format requirement by the KRX format
  specification or by `KER_META_V0_9.xsd` itself.
- **Service** — a Magyar Posta service rule; binding for that service only,
  never generalisable to other KRX producers.
- **Example** — observed in an official sample or figure; illustrative,
  not a stated requirement.
- **Unresolved** — no primary source settles the rule; must not be
  implemented as a requirement.

Source keys used in the tables are defined in
[references.md](references.md): `KRX-SPEC` (KRX message format
specification, 2017-01-16), `HK-2019` (Hivatali kapu technical guide,
2019-07-16, and the schema/sample it embeds), `MKR-2.27`
(Másolatkészítési rend v2.27), `BKSZ-2.1` (BKSZ user manual II v2.1).

## Archive-level rules

| # | Rule | Class | Citation |
| --- | --- | --- | --- |
| A1 | The package is a ZIP archive with the `.krx` extension | Normative | `KRX-SPEC` §1, §5 (p. 2) |
| A2 | The archive must begin with a `mimetype` entry (first ZIP entry) whose content is `application/OCD+ZIP` | Normative | `KRX-SPEC` §4 (p. 2), §6 (p. 3); `HK-2019` schema annotation |
| A3 | Validation checks the `mimetype` content, then reads attachment locations from the metadata and verifies that each listed attachment exists | Normative | `KRX-SPEC` §6 (p. 3) |
| A4 | Descriptive metadata lives in a `Metalayer` directory; today exactly one document, `KULDEMENY_META.xml`, is expected there | Normative | `KRX-SPEC` §4, §5 (p. 2); `HK-2019` schema annotation |
| A5 | Attachments live under `Payload/` in one subdirectory per attachment, each holding one arbitrary-format file; the attachment count is otherwise unbounded. The subdirectory naming is not settled: see A22 | Normative | `KRX-SPEC` §3, §5 (p. 2); `HK-2019` schema annotation |
| A6 | Two attachments may share a file name if they sit in different payload subdirectories | Service | `MKR-2.27` §3.1.2 (p. 24); `BKSZ-2.1` §3.8.2 (p. 26) |
| A7 | An optional `signatures.xml` may accompany the metadata; it carries per-attachment digests (SHA-256/384/512) in XAdES form and is not required on every package | Normative | `KRX-SPEC` §5 (p. 2), §7 (p. 3) |
| A8 | ZIP entry names must use `/` separators per APPNOTE and must not carry a root-directory marker; archives from producers that violate this are not accepted and surface as "attachments not found" | Service | `MKR-2.27` §3.1.2 (p. 24) |
| A9 | The container may carry further descriptive information, which the hybrid conversion ignores | Service | `MKR-2.27` §3.1.2 (p. 24) |
| A10 | Payload subdirectories are `ID-1`…`ID-4`, at most 10 | Service | `BKSZ-2.1` §3.8.2 (p. 26) |
| A11 | Hybrid delivery requires `DeliveryInstruction.xml` in the package, governed by `DeliveryInstruction.xsd` | Service | `KRX-SPEC` §8 (p. 3) |
| A12 | A signed delivery instruction is submitted as `DeliveryInstruction.xml.asice`; the package is rejected if both `DeliveryInstruction.xml` and `DeliveryInstruction.xml.asice` are present | Service | `MKR-2.27` (p. 28) |
| A13 | `Cheques.xml` may be placed in an `ID`-prefixed subdirectory when needed | Service | `MKR-2.27` §3.1.2 (p. 24) |
| A14 | Envelope images arriving over Hivatali kapu must be `logo1.<ext>` and `picture1.<ext>` in the `Metalayer` directory, `.jpeg` or `.png` only | Service | `MKR-2.27` (pp. 62–63) |
| A15 | A consignment-removal request carries `ControlMessage.xml` in the `Metalayer` subdirectory and the referenced dispatch certificate in `Payload/ID_1` | Service | `MKR-2.27` (p. 71) |
| A16 | BKSZ/KSZ messages sent from Hivatali kapu currently carry addressing in a `message.properties` file in the `Metalayer` directory (keys `sender_contract`, `sender_address`, `request_id`, `service`, `recipient_addresses`); merging it into the metadata XML is stated as future work | Service | `BKSZ-2.1` §4.3.1.2 (pp. 53–54) |
| A17 | The `.krx` file name starts with the sender's KÉR root partner identifier; the characters between it and the `.krx` extension are free, up to 20 characters; the name always ends in `.krx` | Service | `KRX-SPEC` §9 (p. 3) |
| A18 | Certificates returned over Hivatali kapu place the certificate PDF in `Payload/ID-1` | Example | `BKSZ-2.1` §4.3.2.2 (pp. 66–67) |
| A19 | Directory nesting and the location of `mimetype` | **Unresolved** | Three primary sources disagree; see [Unresolved rules](#unresolved-essential-rules) |
| A20 | Compression method, extra fields, and byte-exactness required for the `mimetype` entry | **Unresolved** | No source states them |
| A21 | ZIP entry name character encoding (UTF-8 general-purpose flag versus CP437) and case sensitivity | **Unresolved** | No source states them |
| A22 | Payload subdirectory naming: `ID-<n>`, `ID<n>` and `ID_<n>` all appear, as does the numbering base and whether the names must be contiguous | **Unresolved** | Primary sources disagree; see [Unresolved rules](#unresolved-essential-rules) |

## Metadata rules

All rows below come from `KER_META_V0_9.xsd` unless stated otherwise. The
schema is published as an embedded object inside `HK-2019`; `MKR-2.27`
(p. 128) states the ORFK-maintained schema is otherwise supplied by ORFK
developers on request.

| # | Rule | Class | Citation |
| --- | --- | --- | --- |
| M1 | Target namespace `http://xsd.orfk.hu/rzs/ker/kuldemeny`, `elementFormDefault="qualified"`, root element `KULDEMENY` | Normative | `HK-2019` schema |
| M2 | `KULDEMENY` children, in order: `FEJRESZ` (required), then optional `ERKEZTETES`, `BONTASOK`, `EXPEDIALASOK`, `TERTIVEVENY` | Normative | `HK-2019` schema |
| M3 | `FEJRESZ` requires `KRX_VERZIOSZAM`, `FORRASRENDSZER_AZONOSITO`, `KULDEMENY_AZONOSITO`, `KULDEMENY_LETREHOZASANAK_IDEJE` (`xs:dateTime`), `KULDEMENY_TIPUS`, `TESZT` (`xs:boolean`, default `false`); `VONALKOD`, `KULDEMENY_HIVATKOZASI_AZONOSITO`, `HIBAKOD`, `KULDEMENY_MEGJEGYZES` are optional | Normative | `HK-2019` schema |
| M4 | `FORRASRENDSZER_AZONOSITO` is an enumeration (`NOVA`, `KIR3`, `KER`, `POSTA`, `IMAP`); `KULDEMENY_TIPUS` is an enumeration (`KULDEMENY`, `NYUGTA`, `EXPEDIALAS`, `TERTIVEVENY`, `HIBAJELZES`) | Normative | `HK-2019` schema |
| M5 | An attachment reference (`MELLEKLET`) requires `MELLEKLET_LEIRASA`, `CSATOLMANY_SZAMA` (`xs:long`), `FAJL_NEV`, `MERET` (`xs:double`), `ELHELYEZKEDES` (`xs:string`); `MENNYISEG` and `MENNYISEGI_EGYSEG` are optional | Normative | `HK-2019` schema |
| M6 | `MERET` is documented as kilobytes; `ELHELYEZKEDES` is the attachment's location inside the packed KRX file and defaults to `KRX/OCD/Payload` | Normative | `HK-2019` schema |
| M7 | `EXPEDIALAS` requires `MELLEKLETEK_SZAMA` alongside the `MELLEKLETEK` list, so the declared count and the listed references can disagree and must both be checked | Normative | `HK-2019` schema |
| M8 | `KEZELESI_UTASITASOK` inside `EXPEDIALAS` is declared `form="unqualified"` while the schema is otherwise qualified, so that one element is namespace-unqualified | Normative | `HK-2019` schema |
| M9 | Official metadata examples are serialised with `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>` and bind the target namespace to the `ns2` prefix | Example | `HK-2019` sample; `MKR-2.27` appendix 9 (pp. 127–128) |
| M10 | Example `ELHELYEZKEDES` values name the payload directory (`KRX/OCD/Payload/ID-1`) while the file name sits in `FAJL_NEV`, so resolution is a join of the two | Example | `HK-2019` sample; `MKR-2.27` appendix 9 (p. 128) |
| M11 | The `MKR-2.27` appendix 9 metadata example does not validate against `KER_META_V0_9.xsd`: it omits the required `TESZT` and `MELLEKLET_LEIRASA` elements and writes `MERET` as the string `60110 bytes` rather than a number | **Unresolved** | `MKR-2.27` appendix 9 (pp. 127–128) versus `HK-2019` schema |
| M12 | Metadata file name casing: `KULDEMENY_META.xml` in `KRX-SPEC` and `MKR-2.27` appendix 9 versus `kuldemeny_meta.xml` in `BKSZ-2.1` and elsewhere in `MKR-2.27` | **Unresolved** | `KRX-SPEC` §4; `BKSZ-2.1` §3.8.2 (p. 26); `MKR-2.27` (p. 133) |
| M13 | Whether `MERET` carries kilobytes or bytes, and its rounding | **Unresolved** | `HK-2019` schema versus `MKR-2.27` appendix 9 |
| M14 | Whether `ELHELYEZKEDES` is an archive-root-relative path, and how it maps onto real entry names given A19 | **Unresolved** | No source reconciles the two |
| M15 | Which metadata fields a receiving service actually requires beyond schema validity ("addressing data checks" in `KRX-SPEC` §6 are unspecified) | **Unresolved** | `KRX-SPEC` §6 (p. 3) |

## Unresolved essential rules

A19 is the central one. Three primary sources describe three different
layouts, and none of them is marked as superseding the others:

- `KRX-SPEC` §5 (p. 2) shows a top-level `KRX/` directory containing
  `mimetype` and `OCD/`, with `Metalayer/` and `Payload/` inside `OCD/`.
- The `KER_META_V0_9.xsd` annotation in `HK-2019` shows `mimetype` at the
  archive root, a sibling `KRX/` directory, and `OCD/`, `Metalayer/` and
  `Payload/` listed at the same indentation level under it.
- `MKR-2.27` §3.1.2 (pp. 23–24) places `mimetype` in the `OCD` directory
  underneath the top-level `KRX` directory.

A2 requires `mimetype` to be the first ZIP entry, which is difficult to
reconcile with a path that is nested two levels deep, and A8 forbids a
root-directory marker, which is difficult to reconcile with a mandatory
`KRX/` root directory. No retrievable source resolves this.

A22, payload subdirectory naming, is similarly unsettled: `ID-1`, `ID-2` in
`KRX-SPEC` §5 and in both metadata examples, `ID1…IDn` in `MKR-2.27`
§3.1.2 (p. 24) and in `HK-2019`, and `Payload/ID_1` in `MKR-2.27` (p. 71).
`BKSZ-2.1` §3.8.2 (p. 26) adds a maximum of 10 as a service rule (A10), so
even the permitted range is only known for one service.

Because A19, A20, A21, A22, M11, M12, M13 and M14 are open, openKRX cannot
claim structural conformance (KRX-03) or produce a package that can be
asserted to conform (KRX-06). A reader may report what it observed; it may
not report that an archive is a valid KRX package, and a writer must not be
built until the layout is settled by a citable source or by an
authoritative statement obtained from the format owner.

## What is nevertheless usable now

KRX-02 needs only archive-level facts that no source contradicts: the
container is a ZIP archive (A1), entry names use `/` separators (A8), a
marker entry names the format (A2), and real packages carry a small number
of entries (A5, A10). Whether entry names carry a `KRX/` root prefix is
unresolved (A19), so the inventory must not assume either shape. Those
facts support a bounded, profile-agnostic inventory with concrete limits,
provided the inventory reports observations only and never labels an
archive a conforming KRX package.

KRX-03 may implement the metadata grammar in M1–M8 as *schema-shaped
parsing of `KER_META_V0_9`* and must treat M11–M15 as unknown: mismatched
`MERET` units, a missing `TESZT`, a lower-case metadata file name, or an
unexpected `ELHELYEZKEDES` prefix must map to a distinct "unsupported or
unknown" outcome rather than to "invalid".

The checks implemented on that basis, and the rule each unresolved outcome
cites, are listed in
[architecture.md](architecture.md#structural-check-inventory).

## Conformance evidence

Publicly available and independently usable:

- `KER_META_V0_9.xsd` and two official `KULDEMENY_META.xml` samples
  (successful and unsuccessful acceptance), published as embedded objects
  inside `HK-2019` on posta.hu. These support metadata-level checks only.

Missing, and required before any conformance claim:

- No public sample `.krx` archive was found, so no archive-level rule can
  be checked against a real producer's output.
- No independent reference implementation, test suite, or public
  conformance corpus for KRX was found.
- The upstream SPOCS OCD container specification is not retrievable: the
  URL `MKR-2.27` cites on joinup.ec.europa.eu now redirects to
  interoperable-europe.ec.europa.eu and returns HTTP 404. Its logical
  structure is described only second-hand in academic papers.
- No public statement of which KRX versions other than `v0.9` are accepted.

Until an official sample archive or an independent implementation exists,
openKRX fixtures must remain independently authored synthetic archives
whose expected behavior is derived from the cited rules above, and their
agreement with a real service remains unverified.

## Redistribution terms

- posta.hu documents (`KRX-SPEC`, `MKR-2.27`, `BKSZ-2.1`, `HK-2019`,
  including the schema and samples embedded in `HK-2019`): the site states
  only "© Magyar Posta Zrt. Minden jog fenntartva!". No redistribution
  licence was found. Link only; do not vendor, and do not copy at length.
- epapir.gov.hu: states "Minden jog fenntartva ©". No redistribution
  licence was found. Link only.
- SPOCS OCD asset on the European Commission catalogue: page unavailable
  (HTTP 404), so no licence could be recorded.

Nothing from these sources is committed to this repository. Element names,
enumeration values, and directory names are recorded above as facts needed
to describe the format, not as reproduced document text.
