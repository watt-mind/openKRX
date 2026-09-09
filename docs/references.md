# Format evidence and references

Status checked: 2026-09-09. This is an evidence register, not a complete KRX
specification or a declaration of service interoperability. Do not implement
unresolved profile details from conversational examples.

Rules extracted from these sources, with their evidence classification and
the rules that remain unresolved, are in [profile.md](profile.md). Source
keys below (`KRX-SPEC`, `MKR-2.27`, `BKSZ-2.1`, `HK-2019`) are the keys that
document cites. The keys introduced by the 2026-09-09 upstream search
(`OCD-JOINUP`, `ISA2-2014`, `SPOCS-D2.2`, `SPOCS-BB`, `KRXGOV-2021`,
`TRID-OCD`) are local to this register; profile.md cites none of them as a
rule's source, because none of them states a rule.

## Primary sources

### KRX message format specification (`KRX-SPEC`)

[KRX üzenetformátum specifikációja][krxspec], annex 3 of the Magyar Posta
hybrid delivery and conversion connection manual, dated 2017-01-16, 3 pages.
It is the only retrieved document that states the format rules as such:
the `mimetype` first-entry requirement and its `application/OCD+ZIP` value,
the `Metalayer`/`Payload` layout with `ID-<n>` attachment directories, the
optional `signatures.xml`, the validation steps, and `.krx` file naming.

Evidence status: retrieved and read in full. Its layout diagram conflicts
with the other two Posta descriptions; see profile.md, A19.

The same annex is also published as a `.docx` file at the sibling URL
[`..._KRX_2017_0116.docx`][krxspecdoc]; its text was extracted on 2026-09-09
and matches the PDF, including the layout listing (`KRX/`, `mimetype`,
`OCD/`, `Metalayer/`, `KULDEMENY_META.xml`, `[signatures.xml]`, `Payload/`,
`ID-1/`, `ID-2/`) and the schema name pattern `KER_META_Vx.xsd`. It carries
no extra structural statement, so it does not resolve A19–A22.

[krxspec]: https://www.posta.hu/static/internet/download/HKKSZ_csatlakozasi_KK_3_melleklet_KRX_2017_0116_a.pdf
[krxspecdoc]: https://www.posta.hu/static/internet/download/HKKSZ_csatlakozasi_KK_3_melleklet_KRX_2017_0116.docx

### Hivatali kapu technical guide and embedded schema (`HK-2019`)

[Technológiai útmutató a Hivatali kapun keresztüli kommunikációhoz][hk2019],
dated 2019-07-16 (`.docx`). Its "Dokumentum típusok" section describes the
two-part KRX layout, and the document embeds `KER_META_V0_9.xsd` together
with two official `KULDEMENY_META.xml` samples as OLE objects.

Evidence status: retrieved; schema and samples read. This is the only
retrieved source that gives a machine-checkable metadata grammar, and the
only public, independently usable conformance material found.

[hk2019]: https://www.posta.hu/static/internet/download/Hivatali_kapu_muszaki_specifikacio_20190716.docx

### Magyar Posta hybrid-service documentation (`MKR-2.27`)

[Hibrid Szolgáltatás másolatkészítési rend, v2.27][posta], dated 2024-06-30,
section 3.1.2, printed pages 23–26, describes KRX as a ZIP directory
structure derived from OCD. It places `mimetype` with value
`application/OCD+ZIP` in the `OCD` directory under the top-level `KRX`
directory, describes `Metalayer` and payload subdirectories, and requires
APPNOTE-conforming `/` separators with no root-directory marker. Appendix 9
(pp. 127–128) contains a metadata example.

Evidence status: retrieved and read. It supports the broad container model
and this service's usage. It is not a complete schema/profile for all KRX
producers, and its appendix 9 example does not validate against
`KER_META_V0_9.xsd` (profile.md, M11). No schema or sample from this
document is vendored here.

[posta]: https://www.posta.hu/static/internet/download/Masolatkeszitesi_rend_HMDACS_hibrid_2_27_a.pdf

### Magyar Posta BKSZ manual (`BKSZ-2.1`)

[BKSZ Felhasználói kézikönyv II, v2.1][bksz], 147 pages. Section 3.8.2
(pp. 25–26) describes the KRX container, the `ID-1`…`ID-4` payload
subdirectories with a stated maximum of 10, and same-named attachments in
different subdirectories. Section 4.3.1.2 (pp. 53–54) documents the
`message.properties` addressing file in `Metalayer`. Section 4.3.2.2
(pp. 66–67) documents certificates returned in `Payload/ID-1`.

Evidence status: retrieved and read on 2026-09-09. The earlier recorded
retrieval failure did not reproduce. It uses the lower-case metadata file
name `kuldemeny_meta.xml`, which conflicts with `KRX-SPEC` (profile.md, M12).

[bksz]: https://bolt.posta.hu/static/internet/download/BKSZ_Felhasznaloi_kezikonyv_II_v2_1_a.pdf

### e-Papír help

The official [e-Papír help][epapir] lists `.ES3`, `.asice`, and `.dosszie`
among electronic signed-document formats. This supports treating document
formats as separate attachment types. It does not specify KRX creation,
a public submission API, or acceptance of externally produced KRX uploads.

Evidence status: retrieved. A service help page is contextual evidence, not
an archive grammar or an interoperability test.

[epapir]: https://epapir.gov.hu/sugo

## Upstream OCD sources

`MKR-2.27` names the SPOCS OCD (Omnifarious Container for e-Documents)
container as the origin of KRX. The searches recorded under
[Searches performed](#searches-performed) recovered the catalogue entry and
the surrounding project material, but not the specification itself.

### OCD catalogue entry, archived copy (`OCD-JOINUP`)

The live [catalogue entry][ocd] on joinup.ec.europa.eu still redirects to
interoperable-europe.ec.europa.eu and returns HTTP 404 (checked
2026-09-09), but the page itself is archived. The Wayback Machine holds
captures from 2014-12-20 to 2015-12-22; the
[2015-12-22 capture][ocdwayback] was retrieved and read on 2026-09-09.

It states that the SPOCS e-Documents building block defined OCD as a
container of three layers — a payload layer for any electronic document, a
metadata layer, and a common authentication layer — packaged "in ZIP for
PDF files", and that the metadata layer is an XML document whose schema was
derived from the PEPPOL VCD schema following UBL 2.1 naming and design
rules. It lists one distribution, "XML schema for the metadata layer of the
OCD (ZIP)", behind a Joinup login even in the capture.

Evidence status: retrieved from an archive copy. It settles no archive-level
rule: it names no directory, no entry name, no `mimetype` location, no
compression method and no encoding rule. It does establish that the
upstream OCD metadata layer is a UBL/VCD derivative and therefore a
different grammar from `KER_META_V0_9`, so recovering the OCD specification
would not by itself settle M11–M15 either.

[ocd]: https://joinup.ec.europa.eu/catalogue/asset_release/ocd-omnifarious-container-e-documents
[ocdwayback]: https://web.archive.org/web/20151222232646/https://joinup.ec.europa.eu/catalogue/asset_release/ocd-omnifarious-container-e-documents

### European Commission ISA study, 2014 (`ISA2-2014`)

[Analysis of structured e-Document formats used in Trans-European
Systems][isa2], prepared for the ISA Programme by PwC EU Services, section
I.11 and Table 22 (printed pages 137–142). Retrieved and read 2026-09-09.

It repeats the three-layer description, and adds four facts not recorded
before: the OCD licensing framework is the European Union Public Licence
v1.1, classified in the study as "free to use, redistribute, and modify via
non-copyleft licence"; the authoritative source for the schema artefacts is
given as `https://joinup.ec.europa.eu/svn/spocs/semantic-validator`, which
redirects and returns HTTP 404 (checked 2026-09-09); SPOCS published two
modules that create, extract and verify ZIP- and PDF-based OCD containers;
and there is no conformance certification process, no application profile,
and no Schematron rule set for OCD.

Evidence status: retrieved and read. It is a commissioned study, not a
specification: it states the licence and the governance of OCD, not the
container's byte layout. Its own disclaimer states that the views are the
authors' and not an official Commission position, and no reuse notice was
printed on the pages read, so it is linked and not vendored.

[isa2]: https://ec.europa.eu/isa2/sites/isa/files/miscellaneous/analysis-of-structured-e-document-formats-used-in-trans-european-systems_en.pdf

### The OCD specification itself (`SPOCS-D2.2`), still not retrieved

The specification is SPOCS deliverable D2.2, *Standard Document and
Validation Common Specifications*, by K. Stranacher, T. Rössler, G. Marzola,
A. Esposito, T. Kawecki, S. Paoletti, P. Petkus, Y. Katsikogiannis,
N. Routzouni, A. Stasis, P. Milani and G. Degani, 2010. The
[Graz University of Technology research record][d22] confirms the title,
authors and year but carries no full text, no DOI and no licence statement
(checked 2026-09-09).

The download URL the SPOCS project used,
`joinup.ec.europa.eu/site/spocs/eDocuments/references/D2.2_Standard_document_and_validation_common_specifications.zip`,
is dead, and the Wayback Machine holds only HTTP 301 and 404 captures of it
(2024-04-11, 2025-03-22). The project's own site required registration in a
stakeholder group to download deliverables, and no deliverable file appears
in the archived eu-spocs.eu capture set.

Evidence status: identified but not retrieved. This is the single document
most likely to settle A19–A21, and obtaining it is a human task; see
profile.md, [Conformance evidence](profile.md#conformance-evidence).

[d22]: https://tugraz.elsevierpure.com/de/publications/spocs-d22-standard-document-and-validation-common-specifications

### SPOCS eDocuments reference implementation (`SPOCS-BB`)

The SPOCS Starter Kit "eDocuments Building Block", version 4.0.0, last
published 2012-12-20, was a Java implementation with the modules OCD Common,
OCD Extraction, OCD Authentication (plus web service), OCD Processing (plus
web service), OCD API Release, OCD WS Release and OCD Payload ADOC Verifier.
Its generated project site is archived; the
[module index][spocsbb] and the OCD Extraction
[licence page][spocsbblic] were read on 2026-09-09, and the latter states
the project licence as EUPL v1.1 and reproduces the licence text.

Evidence status: identified, not obtained. Only the generated documentation
pages are archived: `download.html`, `gettingstarted.html`,
`architecture.html` and `specification.html` are not in the Wayback capture
set, the Maven Central coordinate search for `spocs` returns no artefacts,
and the SVN repository named as the authoritative source is gone. The
archived API and cross-reference pages cover the metadata data model
classes, not the container packaging code, so nothing in them states a ZIP
layout.

This is the closest thing found to an independent reference implementation,
and its licence would permit reuse if the code were recoverable.

[spocsbb]: https://web.archive.org/web/2015/http://joinup.ec.europa.eu/site/spocs/eDocuments/index.html
[spocsbblic]: https://web.archive.org/web/2015/http://joinup.ec.europa.eu/site/spocs/eDocuments/OCDExtraction/license.html

## Derived and third-party descriptions

These are not primary sources and change no rule's evidence class. They are
recorded because they are the only material found that reports what real
`.krx` archives actually contain, and because a later contributor would
otherwise find them and mistake them for specifications.

### Government tárhely user guide (`KRXGOV-2021`)

*KRX-küldemények kezelése*, a four-page guide to opening a `.krx` file
downloaded from the government storage service. Its original address,
`regi.ugyintezes.magyarorszag.hu/dokumentumok/krx.docx`, no longer resolves
(checked 2026-09-09); the document is republished unchanged by several
bodies, and the copy read was the [Diósd municipal PDF][krxgov], whose
embedded creation date is 2021-04-14.

Its prose repeats the `Metalayer`/`Payload` description already recorded.
What is new is that it contains screenshots of a real archive opened in
Windows Explorer: one shows the address path `Dokumentum.zip` → `KRX` →
`OCD` → `Payload` containing a single folder `ID-1`, another shows that
folder holding one PDF, and a third lists `ID-1`, `ID-2`, `ID-3`, `ID-4`.

Evidence status: retrieved; screenshots examined. This is observation of a
package produced by the government storage service rather than by the
Magyar Posta hybrid service, so it corroborates the `KRX/OCD/…` shape and
the hyphenated `ID-<n>` form from a second producer. It remains derived
evidence: a screenshot in a user guide states no rule, none of the
screenshots shows the archive root, and the location of `mimetype` is
therefore still unobserved. Rules A19 and A22 stay unresolved.

No copyright or reuse statement is attached to the document. Nothing from it
is copied here, and the screenshots contain a natural person's name, so they
are neither reproduced nor described in more detail than the directory names
above.

[krxgov]: https://diosd.asp.lgov.hu/sites/diosd/files/imce/2022-02/krx-csomag-megnyitasa.pdf

### Recipient-side guides

Three further guides written by recipients of real packages describe the
same shape and were retrieved on 2026-09-09: a
[Budapest XVII district guide][rakosmente], which spells the location as
`KRX\OCD\…` with examples `ID-1, ID-2, ID-3`; an
[Onga municipal guide][onga], section 2.6.1, which describes the payload
layer as numbered `ID-1, ID-2, ID-3` folders; and an
[MKFE news item][mkfe], which says only that the package is a ZIP of several
folders. All state "minden jog fenntartva" or carry no notice; link only.

[rakosmente]: https://rakosmente.hu/api/uploads/e_uegyintezes_KRX_megnyitasa_c7a6e38f03.pdf
[onga]: https://www.onga.hu/index.php?module=docs&action=getfile&id=5284
[mkfe]: https://mkfe.hu/hu/mediamenu/hirek/belf%C3%B6ldi-h%C3%ADrek/14995-utmutato-krx-allomanyok-megnyitasahoz.html

### TrID format signature (`TRID-OCD`)

The file-type identification database TrID carries a definition
"Omnifarious Container for eDocuments (generic)" with MIME type
`application/OCD+ZIP`, authored by Marco Pontello and mirrored in the
digital-preservation registry at
[digipres.github.io][trid]. Retrieved and read 2026-09-09.

The definition records that it was generated by `TrIDScan` on 2024-10-01
from six sample files. It declares one front-block pattern, the bytes
`50 4B 03 04` at position 0, and the global strings `METALAYER`,
`MIMETYPE`, `PAYLOAD`, `.XML` and `ID-1`.

Evidence status: retrieved. It is machine-derived from six real archives by
a third party, which makes it the only independent corroboration found that
entries with those names occur together and that such archives begin with a
local file header at offset zero. It settles nothing: TrID stores its
strings upper-cased, so it carries no casing evidence for A21; it records
no path, so it carries none for A19; and it records no compression method,
extra field or entry order, so it carries none for A20.

[trid]: https://github.com/digipres/digipres.github.io/blob/00b9aea89172fde594c6e0c0d654f2204a286162/_sources/registries/trid/triddefs_xml/defs/o/ocd-gen.trid.xml

### A closed-source unpacker

The Hungarian desktop application BLANKETTA offers a "KRX kicsomagolás"
function, so at least one independent implementation exists. Its
[help document][blanketta] was retrieved on 2026-09-09 and documents the
user interface only, stating no layout rule. The program is proprietary and
its source is not published, so it is not usable as conformance evidence.

[blanketta]: https://arconsult.hu/blanketta/sugo.pdf

## Searches performed

Recorded so that the absence below is not mistaken for a cursory look. All
searches were run on 2026-09-09, in Hungarian and English, over web search,
the Wayback Machine CDX index, GitHub code and repository search, Maven
Central, and PRONOM.

Queries included: `KRX fájl Hivatali kapu ZIP szerkezet Metalayer Payload
minta`; `KRX csomag OCD mimetype application/OCD+ZIP`; `minta/példa .krx
letölthető teszt küldemény csomag KRX fejlesztőknek`; `NISZ KRX
specifikáció Hivatali kapu fejlesztői dokumentáció`; `KÉR Központi
Érkeztető Rendszer KRX csatlakozási dokumentáció ORFK KER_META xsd`;
`KRX-küldemények kezelése / Röviden a KRX-csomagról`; `SPOCS Omnifarious
Container for e-Documents OCD specification`; `SPOCS deliverable D2.2
Standard document and validation common specifications`; `OCD container
Metalayer Payload mimetype application/OCD+ZIP specification SPOCS`;
`PRONOM Omnifarious Container / OCD+ZIP / .krx signature`.

What was searched for and **not** found:

- No public sample `.krx` archive is downloadable anywhere that was found.
  Search engine results that appear to be `.krx` file names on
  magyarorszag.hu are submission-receipt URLs containing private personal
  data; they were not opened, retained, or recorded, and they are not
  samples in any case.
- No open-source KRX or OCD implementation. GitHub code search returns zero
  hits for `KULDEMENY_META.xml` and for `KER_META_V0_9`, and zero
  repositories for the KRX terms. The only two code hits for
  `application/OCD+ZIP` are the TrID signature database above and a copy of
  it.
- No conformance corpus, validator service, or test suite.
- No PRONOM or DROID format record for KRX or OCD, so no container
  signature with byte offsets exists there.
- No statement by Magyar Posta, ORFK, NISZ or any government developer
  portal that resolves the layout conflict in A19, and no published
  developer documentation for the KÉR interface; the connection process is
  described as an agreement handled by e-mail.

## Research gate before implementation

Record the supported producer/profile, version, namespaces, metadata grammar,
path casing/root rules, required entries, encoding rules, and attachment
reference resolution. Distinguish normative rules, service-specific rules,
observed examples, and unresolved assumptions. Link each rule to a public
source section; do not generalise one service's requirements to every KRX.

Find independently usable conformance evidence, such as a lawfully usable
synthetic producer sample or an independent reference implementation. A
reader/writer round-trip only proves agreement between our own components.
Do not upload test packages to a real government endpoint during research.

## Copyright and provenance

Links do not grant redistribution rights. Check explicit licence terms
before committing third-party XSDs, documentation, or examples. Prefer an
original implementation and independently authored synthetic fixtures.
Record fixture authorship and purpose in the fixture inventory. This
register does not constitute legal clearance or a trademark assessment.

Terms recorded on 2026-09-09: posta.hu states only
"© Magyar Posta Zrt. Minden jog fenntartva!" and epapir.gov.hu states
"Minden jog fenntartva ©"; neither grants redistribution.

The upstream terms are now partly known. `ISA2-2014` states that the OCD
licensing framework is EUPL v1.1, and the archived SPOCS project site states
the same licence for the eDocuments implementation modules; EUPL v1.1 does
grant redistribution and modification. That licence covers the SPOCS
software and its schema artefacts, none of which could be obtained, so it
changes nothing that is committed here today; it is recorded because it
means recovering `SPOCS-D2.2` or the module sources would give material
this project may lawfully use.

The remaining new sources grant nothing: `ISA2-2014` prints no reuse notice
on the pages read, `KRXGOV-2021` and the recipient guides state
"minden jog fenntartva" or nothing, the TrID definition is linked rather
than copied, and BLANKETTA is proprietary.

Accordingly no third-party document, schema, sample, or archive image is
committed to this repository.
