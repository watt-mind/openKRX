//! What a caller asks for, and the derivation the writer performs on it.
//!
//! A [`PackageSpec`] is a pure value: typed metadata, attachment bytes and a
//! caller-supplied timestamp. Nothing here reads a clock, opens a path or
//! consults the environment, so two callers holding equal specs write equal
//! bytes.
//!
//! The derivation is the part that must not surprise anyone. The writer places
//! each attachment itself and derives the `MELLEKLET` reference that describes
//! it — number, location, file name and size — from that placement. A
//! caller-supplied reference is therefore checked against the derived one and
//! never used instead of it: a document that describes a different package
//! than the one written would be exactly the defect openKRX exists to detect.

use crate::metadata::{AttachmentReference, Dispatch, Metadata};

use super::error::{CreateError, CreateLimitKind, InvalidKind};
use super::names;

/// The archive layout a package is written in.
///
/// The enum is `#[non_exhaustive]` on purpose. One layout exists today, the one
/// `docs/profile.md` describes as canonical, and it is **unverified against any
/// real producer**: rules A19 to A22 are unresolved, so a second variant may
/// have to be added the day a citable source or the format owner settles them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Layout {
    /// `KRX/OCD/mimetype`, `KRX/OCD/Metalayer/KULDEMENY_META.xml` and
    /// `KRX/OCD/Payload/ID-<n>/<file>`, the shape `docs/profile.md` documents.
    #[default]
    CanonicalDocumented,
}

/// The MS-DOS modification date and time written into every record.
///
/// The core crate has no clock. A caller supplies this value, and the same
/// value produces the same bytes; the two fields are written verbatim into
/// every local header and central-directory record, so nothing about the
/// machine that wrote the package leaks into it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FixedTimestamp {
    date: u16,
    time: u16,
}

impl FixedTimestamp {
    /// The earliest value the MS-DOS fields can express: 1980-01-01 00:00:00.
    ///
    /// This is the default, and the value to reach for when a package should
    /// carry no time at all.
    pub const EPOCH: Self = Self {
        date: (1 << 5) | 1,
        time: 0,
    };

    /// Take the two MS-DOS fields exactly as they are to be written.
    #[must_use]
    pub const fn from_dos(date: u16, time: u16) -> Self {
        Self { date, time }
    }

    /// Build the two fields from a calendar time the caller decided.
    ///
    /// The MS-DOS fields hold two-second resolution, so an odd second is
    /// rounded **down** to the even second below it rather than rejected.
    ///
    /// # Errors
    ///
    /// Returns `create.invalid.timestamp` when the value falls outside
    /// 1980-01-01 00:00:00 to 2107-12-31 23:59:59, or names a month, day, hour,
    /// minute or second that does not exist. No calendar validation beyond the
    /// field ranges is performed: 31 February is refused, a leap second is not
    /// representable, and nothing here consults a time zone.
    ///
    /// ```
    /// use openkrx_core::create::FixedTimestamp;
    ///
    /// let stamp = FixedTimestamp::from_parts(2026, 1, 2, 3, 4, 5).unwrap();
    /// assert_eq!(stamp.dos_time(), (3 << 11) | (4 << 5) | 2);
    /// assert!(FixedTimestamp::from_parts(1979, 1, 1, 0, 0, 0).is_err());
    /// ```
    pub fn from_parts(
        year: u16,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
    ) -> Result<Self, CreateError> {
        let invalid = CreateError::Invalid {
            kind: InvalidKind::Timestamp,
            index: None,
        };
        if !(1980..=2107).contains(&year)
            || !(1..=12).contains(&month)
            || !(1..=days_in_month(year, month)).contains(&day)
            || hour > 23
            || minute > 59
            || second > 59
        {
            return Err(invalid);
        }
        let date = ((year - 1980) << 9) | (u16::from(month) << 5) | u16::from(day);
        let time = (u16::from(hour) << 11) | (u16::from(minute) << 5) | u16::from(second / 2);
        Ok(Self { date, time })
    }

    /// The MS-DOS date field, as written.
    #[must_use]
    pub const fn dos_date(self) -> u16 {
        self.date
    }

    /// The MS-DOS time field, as written.
    #[must_use]
    pub const fn dos_time(self) -> u16 {
        self.time
    }
}

impl Default for FixedTimestamp {
    fn default() -> Self {
        Self::EPOCH
    }
}

/// Days in one month, for the field-range check only.
const fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => {
            29
        }
        _ => 28,
    }
}

/// One attachment the package is to carry.
///
/// The file name is one path component: the writer decides where the file goes,
/// and a caller never builds a path. The bytes are written verbatim and read
/// back byte-identically; nothing inspects, converts or unpacks them.
///
/// The file name and the description are package content. The `Debug`
/// representation prints them verbatim and must never be logged, persisted or
/// sent through telemetry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentInput {
    /// `FAJL_NEV`: the file name, one component, no separator.
    pub file_name: String,
    /// The file's bytes, preserved exactly.
    pub bytes: Vec<u8>,
    /// `MELLEKLET_LEIRASA`, the human description.
    ///
    /// Its absence is what rule M11 records one official example doing, so it
    /// is optional here too — and a package written without it reports the
    /// `schema_optional_fields` check as unresolved M11, exactly as a package
    /// read from elsewhere would.
    pub description: Option<String>,
    /// `MENNYISEG`, optional.
    pub quantity: Option<String>,
    /// `MENNYISEGI_EGYSEG`, optional.
    pub quantity_unit: Option<String>,
}

impl AttachmentInput {
    /// One attachment with a file name and its bytes, and nothing optional.
    #[must_use]
    pub fn new(file_name: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            file_name: file_name.into(),
            bytes: bytes.into(),
            description: None,
            quantity: None,
            quantity_unit: None,
        }
    }

    /// The same, with `MELLEKLET_LEIRASA` supplied.
    #[must_use]
    pub fn described(
        file_name: impl Into<String>,
        bytes: impl Into<Vec<u8>>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            description: Some(description.into()),
            ..Self::new(file_name, bytes)
        }
    }

    /// `MERET` as rule M6 documents it: kilobytes, rounded up.
    ///
    /// M6 states the unit and nothing states the rounding, and M13 records that
    /// the unit and rounding a receiving service actually expects are
    /// unresolved. A package written here therefore declares kilobytes, and the
    /// `declared_size` check keeps reporting M13 as unresolved over it.
    #[must_use]
    pub fn declared_kilobytes(&self) -> u64 {
        self.bytes.len().div_ceil(1024) as u64
    }
}

/// A package to write: the document, the attachments, the time, the layout.
///
/// Every field is package content. The `Debug` representation prints it
/// verbatim and must never be logged, persisted or sent through telemetry.
#[derive(Debug, Clone, PartialEq)]
pub struct PackageSpec {
    /// The typed metadata document. Its `MELLEKLET` references and
    /// `MELLEKLETEK_SZAMA` are derived from `attachments`; supplying them is
    /// allowed only when they agree exactly.
    pub metadata: Metadata,
    /// The attachments, in the order they are to be numbered and placed.
    pub attachments: Vec<AttachmentInput>,
    /// The MS-DOS date and time written into every record.
    pub timestamp: FixedTimestamp,
    /// The archive layout. One exists, and it is unverified; see [`Layout`].
    pub layout: Layout,
}

impl PackageSpec {
    /// A package carrying `metadata`, no attachment, and the epoch timestamp.
    #[must_use]
    pub fn new(metadata: Metadata) -> Self {
        Self {
            metadata,
            attachments: Vec::new(),
            timestamp: FixedTimestamp::EPOCH,
            layout: Layout::CanonicalDocumented,
        }
    }

    /// The same, carrying `attachments`.
    #[must_use]
    pub fn with_attachments(metadata: Metadata, attachments: Vec<AttachmentInput>) -> Self {
        Self {
            attachments,
            ..Self::new(metadata)
        }
    }
}

/// `ELHELYEZKEDES` for the attachment at `index`: the directory it sits in
/// (A5, and A22's `ID-<n>` spelling).
#[must_use]
pub(crate) fn payload_location(index: usize) -> String {
    format!("{}{}", super::PAYLOAD_PREFIX, index + 1)
}

/// The document the writer will serialise: the caller's, with the references
/// and the count replaced by the derived ones.
///
/// # Errors
///
/// `create.invalid.dispatch_count` when the attachments have no single
/// `EXPEDIALAS` block to be listed in, `create.invalid.reference_mismatch` when
/// a caller-supplied reference or count disagrees with the attachments,
/// `create.invalid.unknown_elements` when the document counted elements this
/// writer cannot reproduce, and `create.unsafe_name.*` when a file name is one
/// the reader or the extraction planner would refuse.
pub(crate) fn derive(spec: &PackageSpec) -> Result<Metadata, CreateError> {
    if spec.metadata.unknown_elements != 0 {
        return Err(CreateError::Invalid {
            kind: InvalidKind::UnknownElements,
            index: None,
        });
    }
    let derived = derive_references(spec)?;
    let mut metadata = spec.metadata.clone();
    match metadata.dispatches.as_mut_slice() {
        [] if derived.is_empty() => {}
        [dispatch] => place(dispatch, derived)?,
        _ => {
            return Err(CreateError::Invalid {
                kind: InvalidKind::DispatchCount,
                index: None,
            });
        }
    }
    Ok(metadata)
}

/// Put the derived references into the one dispatch, checking what was there.
fn place(dispatch: &mut Dispatch, derived: Vec<AttachmentReference>) -> Result<(), CreateError> {
    let mismatch = |index: Option<u32>| CreateError::Invalid {
        kind: InvalidKind::ReferenceMismatch,
        index,
    };
    if !dispatch.attachments.is_empty() {
        if dispatch.attachments.len() != derived.len() {
            return Err(mismatch(None));
        }
        for (index, (supplied, derived)) in dispatch.attachments.iter().zip(&derived).enumerate() {
            if supplied != derived {
                return Err(mismatch(u32::try_from(index).ok()));
            }
        }
    }
    let count = i64::try_from(derived.len()).map_err(|_| mismatch(None))?;
    if dispatch
        .declared_attachment_count
        .is_some_and(|declared| declared != count)
    {
        return Err(mismatch(None));
    }
    dispatch.declared_attachment_count = Some(count);
    dispatch.attachments = derived;
    Ok(())
}

/// One `MELLEKLET` per attachment, describing where the writer puts it (M5, M6).
fn derive_references(spec: &PackageSpec) -> Result<Vec<AttachmentReference>, CreateError> {
    let mut references = Vec::with_capacity(spec.attachments.len());
    for (index, attachment) in spec.attachments.iter().enumerate() {
        let position = u32::try_from(index).map_err(|_| CreateError::OverLimit {
            limit: CreateLimitKind::Entries,
            limit_value: u64::from(u32::MAX),
            observed: None,
            index: None,
        })?;
        names::check_file_name(&attachment.file_name, position)?;
        let name = format!(
            "{}/{}",
            payload_location(index),
            attachment.file_name.as_str()
        );
        names::check_entry_name(&name, Some(position))?;
        let size_text = attachment.declared_kilobytes().to_string();
        references.push(AttachmentReference {
            description: attachment.description.clone(),
            number: index as i64 + 1,
            file_name: attachment.file_name.clone(),
            size_value: size_text.parse::<f64>().ok(),
            size_text,
            location: payload_location(index),
            quantity: attachment.quantity.clone(),
            quantity_unit: attachment.quantity_unit.clone(),
        });
    }
    if !references.is_empty() && spec.metadata.dispatches.len() != 1 {
        return Err(CreateError::Invalid {
            kind: InvalidKind::DispatchCount,
            index: None,
        });
    }
    Ok(references)
}
