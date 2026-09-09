# Format evidence and references

Status checked: 2026-09-09. This is an evidence register, not a complete KRX
specification or a declaration of service interoperability. Do not implement
unresolved profile details from conversational examples.

## Primary sources

### Magyar Posta hybrid-service documentation

[Hibrid Szolgáltatás másolatkészítési rend, v2.27][posta], dated 2024-06-30,
section 3.1.2, printed pages 23–25, describes KRX as a ZIP directory
structure derived from OCD. It describes `KRX/OCD/mimetype` with value
`application/OCD+ZIP`, a `Metalayer` directory and payload subdirectories,
and requires ZIP path separators. Appendix 9 contains a metadata example.

Evidence status: accessed; supports the broad container model and this
service's usage. It is not a complete schema/profile for all KRX producers.
The prose and examples are insufficient to settle every naming/version rule.
No third-party schema or sample from this document is vendored here.

[posta]: https://www.posta.hu/static/internet/download/Masolatkeszitesi_rend_HMDACS_hibrid_2_27_a.pdf

### Magyar Posta BKSZ manual

[BKSZ Felhasználói kézikönyv II, v2.1][bksz] is a candidate source for
submission metadata and attachment mapping.

Evidence status: URL identified; retrieval failed during scaffold research.
Its contents must be retrieved and checked before any implementation rule
is attributed to it. Its filename alone is not verified version evidence.

[bksz]: https://bolt.posta.hu/static/internet/download/BKSZ_Felhasznaloi_kezikonyv_II_v2_1_a.pdf

### e-Papír help

The official [e-Papír help][epapir] lists `.ES3`, `.asice`, and `.dosszie`
among electronic signed-document formats. This supports treating document
formats as separate attachment types. It does not specify KRX creation,
a public submission API, or acceptance of externally produced KRX uploads.

Evidence status: accessed. A service help page is contextual evidence, not
an archive grammar or an interoperability test.

[epapir]: https://epapir.gov.hu/sugo

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
