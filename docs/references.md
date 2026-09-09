# Format evidence and references

Status checked: 2026-09-09. This is an evidence register, not a complete KRX
specification or a declaration of service interoperability. Do not implement
unresolved profile details from conversational examples.

Rules extracted from these sources, with their evidence classification and
the rules that remain unresolved, are in [profile.md](profile.md). Source
keys below (`KRX-SPEC`, `MKR-2.27`, `BKSZ-2.1`, `HK-2019`) are the keys that
document cites.

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

[krxspec]: https://www.posta.hu/static/internet/download/HKKSZ_csatlakozasi_KK_3_melleklet_KRX_2017_0116_a.pdf

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

## Unreachable sources

The upstream SPOCS OCD (Omnifarious Container for e-Documents)
specification, cited by `MKR-2.27` as the origin of KRX, could not be
retrieved: the [catalogue entry][ocd] on joinup.ec.europa.eu redirects to
interoperable-europe.ec.europa.eu and returns HTTP 404 (checked
2026-09-09). No licence terms could therefore be recorded for it, and the
layout conflicts listed in profile.md cannot be resolved against it.

No public sample `.krx` archive and no independent KRX reference
implementation were found.

[ocd]: https://joinup.ec.europa.eu/catalogue/asset_release/ocd-omnifarious-container-e-documents

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
"Minden jog fenntartva ©"; neither grants redistribution. The European
Commission catalogue entry for OCD is unavailable, so its terms are
unknown. Accordingly no third-party document, schema, or sample is
committed to this repository.
