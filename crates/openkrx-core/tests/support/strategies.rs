//! Strategies for the property-based round-trip tests.
//!
//! Everything generated here is a *valid* request unless the property asks for
//! an invalid one: a `PackageSpec` this module produces is one
//! [`openkrx_core::create::package`] accepts under [`Limits::DEFAULT`], so a
//! refusal is a finding rather than a generator accident. Three rules make that
//! true, and each is the reason a range or a character set is shaped the way it
//! is:
//!
//! - **Text.** Every generated value is trimmed and holds only characters XML
//!   1.0 can carry, because the writer refuses `create.invalid.untrimmed_text`
//!   and `create.invalid.text`. `&`, `<` and `>` are generated deliberately, so
//!   that escaping is exercised rather than avoided.
//! - **Names.** A file name is assembled from an alphabet holding no separator,
//!   colon, control or Windows-reserved character, with edge characters that
//!   are never a space or a dot, and a Windows reserved device name is filtered
//!   out. That is the union of the archive layer's rules and the extraction
//!   planner's, which is exactly what the writer enforces.
//! - **Sizes.** A compressible attachment — a run of zero bytes — never exceeds
//!   [`Limits::RATIO_GRACE_BYTES`], because past that point the writer applies
//!   the reader's compression-ratio ceiling and would refuse it. Bytes just
//!   past the grace are generated too, but only as incompressible noise.
//!
//! Nothing is copied from an official sample: `docs/profile.md` records that no
//! redistribution licence exists for the primary sources.

// Two test binaries include this file, and neither uses all of it: the writer
// round trips have no use for an `Edits`, and the repacking properties have
// none for a ceiling to tighten. Every item here is used by one of them.
#![allow(dead_code)]

use openkrx_core::create::{AttachmentInput, FixedTimestamp, PackageSpec};
use openkrx_core::metadata::{
    ConsignmentKind, Metadata, MetadataLimits, SourceSystem, TARGET_NAMESPACE,
};
use openkrx_core::repack::{
    AttachmentAddition, AttachmentReplacement, Edits, HeaderEdits, OptionalEdit,
};
use openkrx_core::{Limits, metadata};
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

/// Cases each property runs when `PROPTEST_CASES` says nothing.
const DEFAULT_CASES: u32 = 64;
/// Largest number of attachments a generated package carries.
const MAX_ATTACHMENTS: usize = 8;
/// Largest number of attachments a generated package repacking accepts carries.
///
/// Smaller than [`MAX_ATTACHMENTS`] because a repacking property writes the
/// package, reads it, writes it again and often does that twice over, so the
/// cost of one case is several times a writer property's.
const MAX_REPACKABLE_ATTACHMENTS: usize = 4;
/// Largest number of attachments one generated edit adds.
const MAX_ADDITIONS: usize = 3;
/// Decoded bytes an entry may produce before the ratio ceiling applies.
const GRACE: usize = Limits::RATIO_GRACE_BYTES as usize;
/// The XML declaration, exactly as the writer emits it.
const DECLARATION: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>";

/// The bounded configuration every property runs under.
///
/// `regressions` is the file a shrunk failing case is persisted to, relative to
/// the crate root, which is a test binary's working directory. The case count
/// is [`DEFAULT_CASES`] unless `PROPTEST_CASES` overrides it, so the suite stays
/// fast by default and can be turned up for a deliberate long run.
#[must_use]
pub fn config(regressions: &'static str) -> ProptestConfig {
    let cases = std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|cases| *cases > 0)
        .unwrap_or(DEFAULT_CASES);
    ProptestConfig {
        cases,
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(regressions))),
        ..ProptestConfig::default()
    }
}

/// A package request with up to [`MAX_ATTACHMENTS`] attachments of any size the
/// writer accepts, including both boundaries of the compression-ratio grace.
pub fn spec() -> BoxedStrategy<PackageSpec> {
    package_spec(0..=MAX_ATTACHMENTS, mixed_bytes().boxed(), Draft::Full)
}

/// The same, always carrying at least one small attachment.
///
/// The properties that mutate a written package or tighten a ceiling need an
/// attachment to point at and gain nothing from a large one, so this keeps the
/// image small enough to write, read and mutate many times over.
pub fn small_spec() -> BoxedStrategy<PackageSpec> {
    package_spec(1..=4, small_bytes().boxed(), Draft::Full)
}

/// A request whose written package `repack::plan` accepts.
///
/// It is [`spec`] with the four things repacking refuses in an *input* held
/// off: the `ERKEZTETES`, `BONTASOK` and `TERTIVEVENY` marker blocks and the
/// unqualified `KEZELESI_UTASITASOK` element are content the reader records
/// the presence of and never reads further, so a package carrying one is
/// refused with `repack.unsupported.opaque_block` before an edit is looked at.
/// Everything else a repacking property cares about is still drawn: both
/// `MELLEKLETEK` container forms, every optional header element, and zero to
/// [`MAX_REPACKABLE_ATTACHMENTS`] attachments of any size the writer accepts.
///
/// Unlike [`spec`], the drafted document always carries its one `EXPEDIALAS`
/// block, including when the request has no attachment: the container-less
/// empty dispatch only exists in that shape, and it is exactly the shape the
/// reader now retains rather than normalises away.
pub fn repackable_spec() -> BoxedStrategy<PackageSpec> {
    package_spec(
        0..=MAX_REPACKABLE_ATTACHMENTS,
        mixed_bytes().boxed(),
        Draft::Repackable,
    )
}

/// Which documented ceiling a request is to be pushed over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitCase {
    /// `max_entries`, counting the marker and the metadata document.
    Entries,
    /// `max_name_bytes`, applied to a written entry name.
    NameBytes,
    /// `max_entry_decoded_bytes`, applied to one entry.
    EntryBytes,
    /// `max_total_decoded_bytes`, applied to every entry together.
    TotalBytes,
    /// `max_archive_bytes`, applied to the assembled image.
    ArchiveBytes,
    /// `max_compression_ratio`, applied past the grace.
    CompressionRatio,
}

/// One ceiling to exceed, drawn uniformly.
pub fn limit_case() -> impl Strategy<Value = LimitCase> {
    prop::sample::select(vec![
        LimitCase::Entries,
        LimitCase::NameBytes,
        LimitCase::EntryBytes,
        LimitCase::TotalBytes,
        LimitCase::ArchiveBytes,
        LimitCase::CompressionRatio,
    ])
}

/// What shape of document a request is drafted around.
///
/// The writer round trip does not care what a document carries: it reproduces
/// the marker blocks and the unqualified `KEZELESI_UTASITASOK` element from
/// what it was given. Repacking refuses all four, because the reader retains
/// their presence and nothing else, so a repacking property needs a draft that
/// never draws one — and two of them need the `MELLEKLETEK` container form
/// decided rather than drawn, which is why it is a variant here instead of a
/// `prop_filter`: only one drawn document in ten has no attachment and no
/// container, and filtering for it exhausts proptest's local reject budget on
/// a long run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Draft {
    /// Draw every block: the request is only ever written and read back.
    Full,
    /// Hold off the blocks repacking refuses, and draw the container form.
    Repackable,
    /// The same, with the `MELLEKLETEK` container present.
    RepackableWithContainer,
    /// The same, with no `MELLEKLETEK` container.
    RepackableWithoutContainer,
}

/// A request whose attachment count is drawn from `count` and whose bytes come
/// from `bytes`, with a document that can hold the derived references.
///
/// The writer needs exactly one `EXPEDIALAS` block to place references in, and
/// refuses attachments with none, so the drafted document grows a dispatch
/// block whenever the request carries an attachment.
///
/// The block it grows may carry the `MELLEKLETEK` container or not: the writer
/// adds one as soon as it has a reference to list, and a request carrying no
/// attachment is written with whichever form the document had (M7).
fn package_spec(
    count: std::ops::RangeInclusive<usize>,
    bytes: BoxedStrategy<Vec<u8>>,
    draft_shape: Draft,
) -> BoxedStrategy<PackageSpec> {
    (
        prop::collection::vec(attachment(bytes), count),
        metadata_draft(),
        (any::<bool>(), any::<bool>()),
        fixed_timestamp(),
    )
        .prop_map(move |(attachments, mut draft, dispatch, timestamp)| {
            match draft_shape {
                Draft::Full => {
                    if !attachments.is_empty() {
                        draft.dispatch = Some(dispatch);
                    }
                }
                shape => {
                    draft.receipt = false;
                    draft.openings = false;
                    draft.return_receipt = false;
                    let container = match shape {
                        Draft::RepackableWithContainer => true,
                        Draft::RepackableWithoutContainer => false,
                        _ => dispatch.1,
                    };
                    // Only the unqualified element goes; the container is set.
                    draft.dispatch = Some((false, container));
                }
            }
            let mut spec = PackageSpec::with_attachments(draft.metadata(), attachments);
            spec.timestamp = timestamp;
            spec
        })
        .boxed()
}

/// One attachment: a name the planner accepts, bytes, and the optional text.
fn attachment(bytes: BoxedStrategy<Vec<u8>>) -> impl Strategy<Value = AttachmentInput> {
    (
        file_name(),
        bytes,
        prop::option::of(text()),
        prop::option::of(text()),
        prop::option::of(text()),
    )
        .prop_map(
            |(file_name, bytes, description, quantity, quantity_unit)| AttachmentInput {
                file_name,
                bytes,
                description,
                quantity,
                quantity_unit,
            },
        )
}

/// A timestamp inside the MS-DOS range, or the epoch the writer defaults to.
pub fn fixed_timestamp() -> impl Strategy<Value = FixedTimestamp> {
    prop_oneof![
        1 => Just(FixedTimestamp::EPOCH),
        4 => (1980_u16..=2107, 1_u8..=12, 1_u8..=28, 0_u8..=23, 0_u8..=59, 0_u8..=59).prop_map(
            |(year, month, day, hour, minute, second)| FixedTimestamp::from_parts(
                year, month, day, hour, minute, second
            )
            .expect("the parts are inside the MS-DOS range"),
        ),
    ]
}

// ------------------------------------------------------------------ content

/// Small, incompressible bytes: the common case, and the cheap one.
fn small_bytes() -> impl Strategy<Value = Vec<u8>> {
    (any::<u64>(), 0_usize..1024).prop_map(|(seed, length)| pseudo_random(seed, length))
}

/// Sizes from nothing to just past the compression-ratio grace.
///
/// The compressible arm stops **at** [`Limits::RATIO_GRACE_BYTES`]: one byte
/// more and the writer would apply the ratio ceiling to a run of zeros, refuse
/// it with `create.over_limit.compression_ratio`, and make a valid-spec
/// property fail for a reason that is not a defect. Past the grace the bytes
/// are noise, whose ratio is about one.
fn mixed_bytes() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        12 => small_bytes(),
        2 => (0_usize..=GRACE).prop_map(|length| vec![0_u8; length]),
        1 => (any::<u64>(), GRACE + 1..GRACE + 64)
            .prop_map(|(seed, length)| pseudo_random(seed, length)),
    ]
}

/// A deterministic xorshift stream, seeded so shrinking has something to move.
///
/// Deflate cannot compress it, which is what makes it usable at any size: the
/// writer's ratio ceiling never fires over it.
fn pseudo_random(seed: u64, length: usize) -> Vec<u8> {
    let mut state = seed | 1;
    (0..length)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 24) as u8
        })
        .collect()
}

// -------------------------------------------------------------------- names

/// Characters a generated file name may start or end with.
///
/// No space and no dot: a component that starts with a space, ends with a space
/// or ends with a dot is one several filesystems silently rewrite, and both the
/// planner and the writer refuse it.
const NAME_EDGE: [char; 16] = [
    'a', 'b', 'q', 'z', 'A', 'M', 'Z', '0', '5', '9', '-', '_', 'á', 'ő', 'ű', 'ñ',
];

/// Characters a generated file name may hold anywhere.
///
/// The additions over [`NAME_EDGE`] are the ones only the edges forbid, plus a
/// few punctuation marks no target platform reserves. The Latin letters are
/// precomposed, so a name is already in NFC and the planner's collision folding
/// leaves it alone.
const NAME_MIDDLE: [char; 22] = [
    'a', 'b', 'q', 'z', 'A', 'M', 'Z', '0', '5', '9', '-', '_', 'á', 'ő', 'ű', 'ñ', ' ', '.', '(',
    ')', '+', '~',
];

/// Windows reserved device names, upper case, without an extension.
const RESERVED_DEVICE_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// A `FAJL_NEV` the writer accepts and the extraction planner would too.
fn file_name() -> impl Strategy<Value = String> {
    (
        prop::sample::select(&NAME_EDGE[..]),
        prop::collection::vec(prop::sample::select(&NAME_MIDDLE[..]), 0..14),
        prop::sample::select(&NAME_EDGE[..]),
    )
        .prop_map(|(first, middle, last)| {
            let mut name = String::from(first);
            name.extend(middle);
            name.push(last);
            name
        })
        .prop_filter("a Windows reserved device name", |name| {
            let stem = name.split('.').next().unwrap_or(name.as_str());
            !RESERVED_DEVICE_NAMES
                .iter()
                .any(|reserved| stem.eq_ignore_ascii_case(reserved))
        })
}

// --------------------------------------------------------------- the document

/// Characters a generated text value may hold.
///
/// `&`, `<` and `>` are here on purpose: the writer escapes them and the reader
/// unescapes them, and a round trip that never saw one would not test that.
const TEXT_CHARACTERS: [char; 22] = [
    'a', 'b', 'q', 'z', 'A', 'M', 'Z', '0', '5', '9', '-', '_', '.', ',', ' ', '&', '<', '>', 'á',
    'ő', 'ű', '€',
];

/// The `FORRASRENDSZER_AZONOSITO` enumeration (M4).
const SOURCE_SYSTEMS: [&str; 5] = ["NOVA", "KIR3", "KER", "POSTA", "IMAP"];
/// The `KULDEMENY_TIPUS` enumeration (M4).
const CONSIGNMENT_KINDS: [&str; 5] = [
    "KULDEMENY",
    "NYUGTA",
    "EXPEDIALAS",
    "TERTIVEVENY",
    "HIBAJELZES",
];

/// Trimmed text holding only characters the writer can carry.
fn text() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(&TEXT_CHARACTERS[..]), 0..10)
        .prop_map(|characters| characters.into_iter().collect::<String>().trim().to_owned())
}

/// An `xs:dateTime` lexical form, which this crate never interprets.
fn date_time() -> impl Strategy<Value = String> {
    (
        2000_u16..2100,
        1_u8..=12,
        1_u8..=28,
        0_u8..=23,
        0_u8..=59,
        0_u8..=59,
        prop::sample::select(vec!["Z", "+01:00", "-05:00", ""]),
    )
        .prop_map(|(year, month, day, hour, minute, second, zone)| {
            format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.000{zone}")
        })
}

/// The values one `KER_META_V0_9` document is to be built from.
///
/// This is the test's own model, not the crate's: `Metadata` and its parts are
/// `#[non_exhaustive]`, so the only way to obtain one is to parse a document,
/// which is also the only way that proves the document a strategy describes is
/// one the reader accepts.
#[derive(Debug, Clone)]
struct MetadataDraft {
    version: String,
    source_system: &'static str,
    consignment_id: String,
    created_at: String,
    consignment_kind: &'static str,
    test: Option<bool>,
    barcode: Option<String>,
    reference_id: Option<String>,
    error_code: Option<String>,
    note: Option<String>,
    receipt: bool,
    openings: bool,
    return_receipt: bool,
    /// One `EXPEDIALAS` block, or no `EXPEDIALASOK` element at all.
    ///
    /// The first flag carries an unqualified `KEZELESI_UTASITASOK` (M8); the
    /// second emits the `MELLEKLETEK` container. Both forms are generated
    /// because M7 makes them different documents: the container is a separate
    /// element from the count, so a dispatch listing nothing may carry an
    /// empty one or none at all, and the writer must reproduce whichever it
    /// was given.
    dispatch: Option<(bool, bool)>,
}

/// Every field of a document, drawn independently.
fn metadata_draft() -> impl Strategy<Value = MetadataDraft> {
    (
        text(),
        prop::sample::select(&SOURCE_SYSTEMS[..]),
        text(),
        date_time(),
        prop::sample::select(&CONSIGNMENT_KINDS[..]),
        prop::option::of(any::<bool>()),
        (
            prop::option::of(text()),
            prop::option::of(text()),
            prop::option::of(text()),
            prop::option::of(text()),
        ),
        (any::<bool>(), any::<bool>(), any::<bool>()),
        prop::option::of((any::<bool>(), any::<bool>())),
    )
        .prop_map(
            |(
                version,
                source_system,
                consignment_id,
                created_at,
                consignment_kind,
                test,
                (barcode, reference_id, error_code, note),
                (receipt, openings, return_receipt),
                dispatch,
            )| MetadataDraft {
                version,
                source_system,
                consignment_id,
                created_at,
                consignment_kind,
                test,
                barcode,
                reference_id,
                error_code,
                note,
                receipt,
                openings,
                return_receipt,
                dispatch,
            },
        )
}

impl MetadataDraft {
    /// The parsed document these values describe.
    ///
    /// Parsing is deliberately not `Result`: a draft that does not parse is a
    /// defect in this module, and a property that reported it as a failing case
    /// would point at the wrong code.
    fn metadata(&self) -> Metadata {
        metadata::parse(self.document().as_bytes(), &MetadataLimits::DEFAULT)
            .expect("a drafted document parses")
    }

    /// Serialise the draft in the M2, M3 and M5 element order.
    fn document(&self) -> String {
        let mut out = String::from(DECLARATION);
        out.push_str(&format!("<ns2:KULDEMENY xmlns:ns2=\"{TARGET_NAMESPACE}\">"));
        out.push_str(&self.header());
        if self.receipt {
            out.push_str("<ns2:ERKEZTETES></ns2:ERKEZTETES>");
        }
        if self.openings {
            out.push_str("<ns2:BONTASOK></ns2:BONTASOK>");
        }
        if let Some((handling, container)) = self.dispatch {
            out.push_str("<ns2:EXPEDIALASOK><ns2:EXPEDIALAS>");
            if container {
                out.push_str("<ns2:MELLEKLETEK></ns2:MELLEKLETEK>");
            }
            if handling {
                // M8: the schema declares this one element unqualified.
                out.push_str("<KEZELESI_UTASITASOK></KEZELESI_UTASITASOK>");
            }
            out.push_str("</ns2:EXPEDIALAS></ns2:EXPEDIALASOK>");
        }
        if self.return_receipt {
            out.push_str("<ns2:TERTIVEVENY></ns2:TERTIVEVENY>");
        }
        out.push_str("</ns2:KULDEMENY>");
        out
    }

    /// `FEJRESZ`, in the M3 order.
    fn header(&self) -> String {
        let mut out = String::from("<ns2:FEJRESZ>");
        out.push_str(&leaf("KRX_VERZIOSZAM", &self.version));
        out.push_str(&leaf("FORRASRENDSZER_AZONOSITO", self.source_system));
        out.push_str(&leaf("KULDEMENY_AZONOSITO", &self.consignment_id));
        out.push_str(&leaf("KULDEMENY_LETREHOZASANAK_IDEJE", &self.created_at));
        out.push_str(&leaf("KULDEMENY_TIPUS", self.consignment_kind));
        if let Some(test) = self.test {
            out.push_str(&leaf("TESZT", if test { "true" } else { "false" }));
        }
        for (name, value) in [
            ("VONALKOD", &self.barcode),
            ("KULDEMENY_HIVATKOZASI_AZONOSITO", &self.reference_id),
            ("HIBAKOD", &self.error_code),
            ("KULDEMENY_MEGJEGYZES", &self.note),
        ] {
            if let Some(value) = value {
                out.push_str(&leaf(name, value));
            }
        }
        out.push_str("</ns2:FEJRESZ>");
        out
    }
}

/// One qualified leaf element carrying `value`, escaped as the writer escapes.
fn leaf(name: &str, value: &str) -> String {
    let escaped = value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!("<ns2:{name}>{escaped}</ns2:{name}>")
}

// -------------------------------------------------------------------- edits

/// One `SourceSystem`, drawn from the M4 enumeration in full.
fn source_system() -> impl Strategy<Value = SourceSystem> {
    prop::sample::select(&SOURCE_SYSTEMS[..])
        .prop_map(|token| SourceSystem::parse(token).expect("a token the enumeration lists"))
}

/// One `ConsignmentKind`, drawn from the M4 enumeration in full.
fn consignment_kind() -> impl Strategy<Value = ConsignmentKind> {
    prop::sample::select(&CONSIGNMENT_KINDS[..])
        .prop_map(|token| ConsignmentKind::parse(token).expect("a token the enumeration lists"))
}

/// What an edit does to one optional header element.
///
/// The three arms are three different edits and none is reachable from
/// another: leaving `VONALKOD` alone, giving it a value and removing it.
/// `Keep` is drawn twice as often because it is what most edits do to most
/// elements, and a generated edit that changed all four every time would never
/// exercise a document keeping one.
fn optional_edit() -> impl Strategy<Value = OptionalEdit> {
    prop_oneof![
        2 => Just(OptionalEdit::Keep),
        1 => text().prop_map(OptionalEdit::Set),
        1 => Just(OptionalEdit::Clear),
    ]
}

/// Any subset of the `FEJRESZ` fields, each with a value the writer accepts.
///
/// Every value comes from the same generators the document draft uses, so a
/// refusal from the writer is a finding rather than a generator accident: the
/// text is trimmed and inside XML 1.0, the two enumerations are the schema's
/// own tokens, and `KULDEMENY_LETREHOZASANAK_IDEJE` is an `xs:dateTime`
/// lexical form this crate never interprets.
pub fn header_edits() -> BoxedStrategy<HeaderEdits> {
    (
        prop::option::of(text()),
        prop::option::of(source_system()),
        prop::option::of(text()),
        prop::option::of(date_time()),
        prop::option::of(consignment_kind()),
        prop::option::of(any::<bool>()),
        optional_edit(),
        optional_edit(),
        optional_edit(),
        optional_edit(),
    )
        .prop_map(
            |(
                version,
                source_system,
                consignment_id,
                created_at_text,
                consignment_kind,
                test,
                barcode,
                reference_id,
                error_code,
                note,
            )| HeaderEdits {
                version,
                source_system,
                consignment_id,
                created_at_text,
                consignment_kind,
                test,
                barcode,
                reference_id,
                error_code,
                note,
            },
        )
        .boxed()
}

/// Attachments to add, as many as `count` says.
///
/// The file names and bytes are the ones [`spec`] would have written, so an
/// addition is refused only for a reason a created package would have been.
pub fn additions(count: std::ops::RangeInclusive<usize>) -> BoxedStrategy<Vec<AttachmentAddition>> {
    prop::collection::vec(
        (file_name(), small_bytes(), prop::option::of(text())).prop_map(
            |(file_name, bytes, description)| AttachmentAddition {
                file_name,
                bytes,
                description,
            },
        ),
        count,
    )
    .boxed()
}

/// An edit list every target of which names an attachment the package holds,
/// once.
///
/// Each of the `attachments` numbers is independently left alone, removed or
/// replaced, which is what makes "no target used twice" true by construction
/// rather than by a filter — and it draws the interesting overlaps for free: a
/// removal below a replacement renumbers it, and removing everything leaves a
/// document declaring none.
pub fn edits_for(attachments: usize) -> BoxedStrategy<Edits> {
    (
        header_edits(),
        prop::collection::vec((0_u8..3, small_bytes()), attachments),
        additions(0..=MAX_ADDITIONS),
    )
        .prop_map(|(header, targets, add)| {
            let mut edits = Edits {
                header,
                add,
                ..Edits::default()
            };
            for (index, (choice, bytes)) in targets.into_iter().enumerate() {
                let number = u32::try_from(index + 1).expect("a small number");
                match choice {
                    1 => edits.remove.push(number),
                    2 => edits.replace.push(AttachmentReplacement { number, bytes }),
                    _ => {}
                }
            }
            edits
        })
        .boxed()
}

/// An edit list `plan` must refuse, with the code it must refuse it under.
///
/// The two arms are the whole `repack.invalid.*` surface a caller can reach
/// through an edit: a number the package does not carry — zero is one, because
/// `CSATOLMANY_SZAMA` counts from 1 — and one number named twice. A package
/// with no attachment has no valid number to duplicate, so only the first arm
/// is drawn for it.
pub fn invalid_edits_for(attachments: usize) -> BoxedStrategy<(Edits, &'static str)> {
    let count = u32::try_from(attachments).expect("a small number");
    let out_of_range = (
        prop_oneof![Just(0_u32), count + 1..=count + 8],
        any::<bool>(),
    )
        .prop_map(|(number, as_replacement)| {
            let mut edits = Edits::default();
            if as_replacement {
                edits.replace.push(AttachmentReplacement {
                    number,
                    bytes: Vec::new(),
                });
            } else {
                edits.remove.push(number);
            }
            (edits, "repack.invalid.no_such_attachment")
        });
    if count == 0 {
        return out_of_range.boxed();
    }
    let duplicate = (1_u32..=count, any::<bool>()).prop_map(|(number, as_replacement)| {
        let mut edits = Edits::default();
        edits.remove.push(number);
        if as_replacement {
            edits.replace.push(AttachmentReplacement {
                number,
                bytes: Vec::new(),
            });
        } else {
            edits.remove.push(number);
        }
        (edits, "repack.invalid.duplicate_target")
    });
    prop_oneof![out_of_range, duplicate].boxed()
}

/// A package repacking accepts whose empty dispatch carries no `MELLEKLETEK`
/// container.
///
/// It is the complement of [`repackable_spec_with_container`], and the only
/// shape an addition changes irreversibly: the writer must emit the container
/// to hold the added reference, and removing the reference again cannot take
/// the container away, because nothing in the package it is given says the
/// original had none. A package holding an attachment cannot be drawn here —
/// the writer would have written the container for it.
pub fn repackable_spec_without_container() -> BoxedStrategy<PackageSpec> {
    package_spec(
        0..=0,
        small_bytes().boxed(),
        Draft::RepackableWithoutContainer,
    )
}

/// A package repacking accepts whose written document carries the
/// `MELLEKLETEK` container.
///
/// That is every package holding an attachment — the writer emits the
/// container as soon as it has a reference to place in it, whatever the
/// document it was handed said — plus the empty dispatch that carried one
/// anyway. Adding an attachment and removing it again is an exact identity
/// over these, which is what makes [`repackable_spec_without_container`] the
/// one shape that needs a property of its own.
pub fn repackable_spec_with_container() -> BoxedStrategy<PackageSpec> {
    prop_oneof![
        3 => package_spec(
            1..=MAX_REPACKABLE_ATTACHMENTS,
            mixed_bytes().boxed(),
            Draft::Repackable,
        ),
        1 => package_spec(0..=0, small_bytes().boxed(), Draft::RepackableWithContainer),
    ]
    .boxed()
}

/// A package repacking accepts, with a valid edit list drawn for its own
/// attachment count.
///
/// The count is only known once the request is drawn, which is why this is a
/// `prop_flat_map` rather than two independent strategies: an edit list drawn
/// first could only name numbers by luck.
pub fn repackable_spec_with_edits() -> BoxedStrategy<(PackageSpec, Edits)> {
    repackable_spec()
        .prop_flat_map(|spec| {
            let attachments = spec.attachments.len();
            (Just(spec), edits_for(attachments))
        })
        .boxed()
}

/// The same, with an edit list `plan` must refuse and the code it must report.
pub fn repackable_spec_with_invalid_edits() -> BoxedStrategy<(PackageSpec, Edits, &'static str)> {
    repackable_spec()
        .prop_flat_map(|spec| {
            let attachments = spec.attachments.len();
            (Just(spec), invalid_edits_for(attachments))
        })
        .prop_map(|(spec, (edits, code))| (spec, edits, code))
        .boxed()
}
