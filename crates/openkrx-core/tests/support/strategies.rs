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

use openkrx_core::create::{AttachmentInput, FixedTimestamp, PackageSpec};
use openkrx_core::metadata::{Metadata, MetadataLimits, TARGET_NAMESPACE};
use openkrx_core::{Limits, metadata};
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

/// Cases each property runs when `PROPTEST_CASES` says nothing.
const DEFAULT_CASES: u32 = 64;
/// Largest number of attachments a generated package carries.
const MAX_ATTACHMENTS: usize = 8;
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
    package_spec(0..=MAX_ATTACHMENTS, mixed_bytes().boxed())
}

/// The same, always carrying at least one small attachment.
///
/// The properties that mutate a written package or tighten a ceiling need an
/// attachment to point at and gain nothing from a large one, so this keeps the
/// image small enough to write, read and mutate many times over.
pub fn small_spec() -> BoxedStrategy<PackageSpec> {
    package_spec(1..=4, small_bytes().boxed())
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

/// A request whose attachment count is drawn from `count` and whose bytes come
/// from `bytes`, with a document that can hold the derived references.
///
/// The writer needs exactly one `EXPEDIALAS` block to place references in, and
/// refuses attachments with none, so the drafted document grows a dispatch
/// block whenever the request carries an attachment.
fn package_spec(
    count: std::ops::RangeInclusive<usize>,
    bytes: BoxedStrategy<Vec<u8>>,
) -> BoxedStrategy<PackageSpec> {
    (
        prop::collection::vec(attachment(bytes), count),
        metadata_draft(),
        any::<bool>(),
        fixed_timestamp(),
    )
        .prop_map(|(attachments, mut draft, handling, timestamp)| {
            if !attachments.is_empty() {
                draft.dispatch = Some(handling);
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
    /// One `EXPEDIALAS` block, carrying an unqualified `KEZELESI_UTASITASOK`
    /// when the flag is set (M8), or no `EXPEDIALASOK` element at all.
    dispatch: Option<bool>,
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
        prop::option::of(any::<bool>()),
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
        if let Some(handling) = self.dispatch {
            out.push_str("<ns2:EXPEDIALASOK><ns2:EXPEDIALAS><ns2:MELLEKLETEK></ns2:MELLEKLETEK>");
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
