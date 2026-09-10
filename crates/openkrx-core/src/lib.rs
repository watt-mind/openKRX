//! openKRX development foundation.
//!
//! The crate implements three reading layers over caller-supplied bytes:
//! [`archive::inventory`], a bounded, profile-agnostic ZIP reader for hostile
//! input; [`metadata::parse`], bounded schema-shaped parsing of a
//! `KER_META_V0_9` document; and [`profile::check`], a fixed inventory of
//! structural checks over the two. A fourth layer writes: [`create::package`]
//! turns typed metadata and attachment bytes into the bytes of one package,
//! deterministically, in the layout `docs/profile.md` documents — a layout that
//! is unverified against every real producer, so what it produces is
//! "structurally consistent with the documented layout" and never "conforming".
//! A fifth composes them: [`repack::plan`] and [`repack::apply`] edit a package
//! that already exists, preserving every attachment no edit names byte for
//! byte and refusing any package the writer cannot re-emit rather than one
//! that silently lost part of it.
//! None of the five performs filesystem, clock, process or network access, and
//! none resolves anything external.
//!
//! The three reader commands `inspect`, `list` and `validate-structure` expose
//! these layers, and [`capabilities`] names them beside the two commands that
//! write: `extract`, where `openkrx-cli` joins [`extract::plan`] onto a
//! caller-selected destination under a no-clobber, no-link, undo-on-failure
//! policy, and `create`, where it turns a manifest file into a [`PackageSpec`]
//! and writes the bytes [`create::package`] returned to a file that must not
//! already exist. A package openKRX wrote is structurally consistent with the
//! documented layout, and is never a conforming one.
//! Nothing here asserts that an archive is a KRX package:
//! `docs/profile.md` lists essential rules as unresolved, and each of them maps
//! to a distinct [`profile::CheckOutcome::Unresolved`] outcome rather than to a
//! pass or a failure.
//!
//! Every refusal is a typed error carrying one stable dotted code, and
//! `docs/codes.md` is the catalogue of them, section by section: archive,
//! metadata, structural check, extraction, creation and repacking codes.
//! `docs/profile.md` holds the profile evidence and the list of essential
//! rules no primary source settles; `docs/conformance.md` states what a
//! structural report is and is not; `docs/architecture.md` is the canonical
//! description of the layers above. Public documentation is a build gate here:
//! this crate denies `missing_docs`, so a public item without a doc comment
//! does not compile.
//!
//! # Examples
//!
//! One example per layer follows, each compiled and run by
//! `cargo test --doc -p openkrx-core`. They pass `Vec<u8>` around and open no
//! path, because no layer of this crate touches a filesystem: a package is
//! bytes here, and where those bytes come from or go is the caller's decision.
//! The package every example works on is built by the public [`draft`] and
//! [`create`] surface — the same one the `create` command uses — so nothing
//! below needs a committed fixture or the test-only `synthetic-writer`
//! feature.
//!
//! The values are synthetic. Package content must never be logged, persisted
//! or sent through telemetry.
//!
//! ## Write a package
//!
//! ```
//! use openkrx_core::create::{self, AttachmentInput, PackageSpec};
//! use openkrx_core::draft::{self, HeaderDraft};
//! use openkrx_core::metadata::{ConsignmentKind, SourceSystem};
//! use openkrx_core::Limits;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let header = HeaderDraft {
//!     version: "0.9".to_owned(),
//!     source_system: SourceSystem::Ker,
//!     consignment_id: "SYNTHETIC-0001".to_owned(),
//!     created_at_text: "2026-01-02T03:04:05".to_owned(),
//!     consignment_kind: ConsignmentKind::Kuldemeny,
//!     test: true,
//!     barcode: None,
//!     reference_id: None,
//!     error_code: None,
//!     note: None,
//! }
//! .build();
//! // One `EXPEDIALAS` block declaring the single attachment below. The
//! // writer derives the reference and the count from the attachments; the
//! // declared count is checked against them, never written in their place.
//! let metadata = draft::metadata(header, vec![draft::dispatch(Some(1))]);
//! let spec = PackageSpec::with_attachments(
//!     metadata,
//!     vec![AttachmentInput::described(
//!         "notice.txt",
//!         b"synthetic attachment".to_vec(),
//!         "A synthetic notice",
//!     )],
//! );
//!
//! let image = create::package(&spec, &Limits::DEFAULT)?;
//! // Deterministic: no clock, no randomness, no environment.
//! assert_eq!(image, create::package(&spec, &Limits::DEFAULT)?);
//! # Ok(())
//! # }
//! ```
//!
//! ## Read one back
//!
//! ```
//! use openkrx_core::{Limits, MetadataLimits, archive, create, metadata};
//!
//! # use openkrx_core::create::{AttachmentInput, PackageSpec};
//! # use openkrx_core::draft::{self, HeaderDraft};
//! # use openkrx_core::metadata::{ConsignmentKind, SourceSystem};
//! # // The package the first example writes, rebuilt so that this one stands
//! # // on its own.
//! # fn example_package() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
//! #     let header = HeaderDraft {
//! #         version: "0.9".to_owned(), source_system: SourceSystem::Ker,
//! #         consignment_id: "SYNTHETIC-0001".to_owned(),
//! #         created_at_text: "2026-01-02T03:04:05".to_owned(),
//! #         consignment_kind: ConsignmentKind::Kuldemeny, test: true,
//! #         barcode: None, reference_id: None, error_code: None, note: None,
//! #     }.build();
//! #     let attachment = AttachmentInput::described(
//! #         "notice.txt", b"synthetic attachment".to_vec(), "A synthetic notice");
//! #     let spec = PackageSpec::with_attachments(
//! #         draft::metadata(header, vec![draft::dispatch(Some(1))]), vec![attachment]);
//! #     Ok(openkrx_core::create::package(&spec, &openkrx_core::Limits::DEFAULT)?)
//! # }
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let image = example_package()?;
//! let inventory = archive::inventory(&image, &Limits::DEFAULT)?;
//! assert!(inventory.is_first_entry_named(create::MARKER_NAME.as_bytes()));
//!
//! let position = inventory
//!     .entries()
//!     .iter()
//!     .position(|entry| entry.name_bytes() == create::METADATA_NAME.as_bytes())
//!     .ok_or("no metadata document")?;
//! let document = inventory.entry_bytes(position as u32)?;
//! let parsed = metadata::parse(&document, &MetadataLimits::DEFAULT)?;
//! assert_eq!(parsed.header.consignment_id, "SYNTHETIC-0001");
//! # Ok(())
//! # }
//! ```
//!
//! ## Run the structural checks
//!
//! ```
//! use openkrx_core::profile::{CheckOutcome, StructureSummary};
//! use openkrx_core::{Limits, MetadataLimits, archive, profile};
//!
//! # use openkrx_core::create::{AttachmentInput, PackageSpec};
//! # use openkrx_core::draft::{self, HeaderDraft};
//! # use openkrx_core::metadata::{ConsignmentKind, SourceSystem};
//! # // The package the first example writes, rebuilt so that this one stands
//! # // on its own.
//! # fn example_package() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
//! #     let header = HeaderDraft {
//! #         version: "0.9".to_owned(), source_system: SourceSystem::Ker,
//! #         consignment_id: "SYNTHETIC-0001".to_owned(),
//! #         created_at_text: "2026-01-02T03:04:05".to_owned(),
//! #         consignment_kind: ConsignmentKind::Kuldemeny, test: true,
//! #         barcode: None, reference_id: None, error_code: None, note: None,
//! #     }.build();
//! #     let attachment = AttachmentInput::described(
//! #         "notice.txt", b"synthetic attachment".to_vec(), "A synthetic notice");
//! #     let spec = PackageSpec::with_attachments(
//! #         draft::metadata(header, vec![draft::dispatch(Some(1))]), vec![attachment]);
//! #     Ok(openkrx_core::create::package(&spec, &openkrx_core::Limits::DEFAULT)?)
//! # }
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let image = example_package()?;
//! let inventory = archive::inventory(&image, &Limits::DEFAULT)?;
//! let report = profile::check(&inventory, &MetadataLimits::DEFAULT)?;
//! assert!(!report
//!     .checks()
//!     .iter()
//!     .any(|check| matches!(check.outcome, CheckOutcome::Fail(_))));
//! // Not `Consistent`: the root prefix (A19) and the declared size (M13) are
//! // unresolved rules, so they are reported as unresolved rather than passed.
//! // A summary is never a conformance verdict.
//! assert_eq!(report.summary(), StructureSummary::Unresolved);
//! # Ok(())
//! # }
//! ```
//!
//! ## Plan an extraction
//!
//! ```
//! use openkrx_core::extract::{self, ExtractLimits};
//! use openkrx_core::{Limits, archive};
//!
//! # use openkrx_core::create::{AttachmentInput, PackageSpec};
//! # use openkrx_core::draft::{self, HeaderDraft};
//! # use openkrx_core::metadata::{ConsignmentKind, SourceSystem};
//! # // The package the first example writes, rebuilt so that this one stands
//! # // on its own.
//! # fn example_package() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
//! #     let header = HeaderDraft {
//! #         version: "0.9".to_owned(), source_system: SourceSystem::Ker,
//! #         consignment_id: "SYNTHETIC-0001".to_owned(),
//! #         created_at_text: "2026-01-02T03:04:05".to_owned(),
//! #         consignment_kind: ConsignmentKind::Kuldemeny, test: true,
//! #         barcode: None, reference_id: None, error_code: None, note: None,
//! #     }.build();
//! #     let attachment = AttachmentInput::described(
//! #         "notice.txt", b"synthetic attachment".to_vec(), "A synthetic notice");
//! #     let spec = PackageSpec::with_attachments(
//! #         draft::metadata(header, vec![draft::dispatch(Some(1))]), vec![attachment]);
//! #     Ok(openkrx_core::create::package(&spec, &openkrx_core::Limits::DEFAULT)?)
//! # }
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let image = example_package()?;
//! let inventory = archive::inventory(&image, &Limits::DEFAULT)?;
//! let plan = extract::plan(&inventory, &ExtractLimits::DEFAULT)?;
//! // Destination components as text: no `PathBuf`, no absolute path and no
//! // platform separator. The caller decides what a path is, and nothing has
//! // been written.
//! assert!(plan.items().iter().all(|item| item
//!     .components()
//!     .iter()
//!     .all(|component| !component.is_empty())));
//! assert!(plan
//!     .items()
//!     .iter()
//!     .any(|item| item.components().last().map(String::as_str) == Some("notice.txt")));
//! # Ok(())
//! # }
//! ```
//!
//! ## Repack it
//!
//! ```
//! use openkrx_core::create::FixedTimestamp;
//! use openkrx_core::repack::{self, AttachmentAddition, Edits};
//! use openkrx_core::{Limits, MetadataLimits, archive};
//!
//! # use openkrx_core::create::{AttachmentInput, PackageSpec};
//! # use openkrx_core::draft::{self, HeaderDraft};
//! # use openkrx_core::metadata::{ConsignmentKind, SourceSystem};
//! # // The package the first example writes, rebuilt so that this one stands
//! # // on its own.
//! # fn example_package() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
//! #     let header = HeaderDraft {
//! #         version: "0.9".to_owned(), source_system: SourceSystem::Ker,
//! #         consignment_id: "SYNTHETIC-0001".to_owned(),
//! #         created_at_text: "2026-01-02T03:04:05".to_owned(),
//! #         consignment_kind: ConsignmentKind::Kuldemeny, test: true,
//! #         barcode: None, reference_id: None, error_code: None, note: None,
//! #     }.build();
//! #     let attachment = AttachmentInput::described(
//! #         "notice.txt", b"synthetic attachment".to_vec(), "A synthetic notice");
//! #     let spec = PackageSpec::with_attachments(
//! #         draft::metadata(header, vec![draft::dispatch(Some(1))]), vec![attachment]);
//! #     Ok(openkrx_core::create::package(&spec, &openkrx_core::Limits::DEFAULT)?)
//! # }
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let image = example_package()?;
//! let inventory = archive::inventory(&image, &Limits::DEFAULT)?;
//! let mut edits = Edits::default();
//! edits.header.consignment_id = Some("SYNTHETIC-0002".to_owned());
//! edits.add.push(AttachmentAddition::new("second.txt", b"another".to_vec()));
//!
//! let plan = repack::plan(&inventory, &Limits::DEFAULT, &MetadataLimits::DEFAULT, &edits)?;
//! assert_eq!(plan.preserved(), &[1]);
//! assert_eq!(plan.added(), &[2]);
//! let edited = repack::apply(&inventory, &plan, FixedTimestamp::EPOCH, &Limits::DEFAULT)?;
//! assert_ne!(edited, image);
//! # Ok(())
//! # }
//! ```

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(rustdoc::private_intra_doc_links)]

use serde::Serialize;

pub mod archive;
pub mod create;
mod error;
pub mod extract;
mod limits;
pub mod metadata;
pub mod profile;
pub mod repack;
#[cfg(feature = "synthetic-writer")]
pub mod synthetic;

pub use create::{CreateError, PackageSpec};
pub use error::{
    AmbiguityKind, ArchiveError, LimitKind, MalformedKind, Structure, UnsafeNameKind,
    UnsupportedKind,
};
pub use extract::{ExtractLimits, ExtractionPlan, PlanError, PlanItem};
pub use limits::Limits;
pub use metadata::{MetadataError, MetadataLimits};
pub use profile::{ProfileError, StructureReport, StructureSummary};
pub use repack::{Edits, RepackError, RepackPlan};

/// Machine-readable implementation status; never a verification verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Capabilities {
    /// Public project name.
    pub project: &'static str,
    /// Current implementation stage.
    pub stage: &'static str,
    /// Implemented document or workflow operations.
    pub operations: &'static [&'static str],
}

/// Return the current implementation status without I/O or side effects.
///
/// The list names the package operations a command actually performs. Reading,
/// protected extraction, deterministic creation and deterministic repacking
/// are implemented, so the stage is `reader-writer`; a structural report is not a conformance verdict
/// whichever command produced it, and `create` writing a package is not a
/// statement that a receiving service would accept it.
/// `extract` appears here only because every filesystem property
/// `SECURITY.md` requires of it is held by a test on each supported platform,
/// and `create` and `repack` only because every package they write is read
/// back by the writer's own structural check before the command reports
/// success.
pub const fn capabilities() -> Capabilities {
    Capabilities {
        project: "openKRX",
        stage: "reader-writer",
        operations: &[
            "inspect",
            "list",
            "validate-structure",
            "extract",
            "create",
            "repack",
        ],
    }
}

/// Constructors for the metadata model a [`PackageSpec`] carries.
///
/// [`metadata::Metadata`] and the types under it are `#[non_exhaustive]`, so a
/// later profile revision can add a field without breaking a reader that
/// matches on them. That also means no crate outside this one can build one
/// with a struct literal, and a caller of [`create::package`] must build one.
/// These constructors are that surface, and they are deliberately narrow: they
/// take the values the documented layout can express and derive nothing, so
/// that everything the writer decides stays in [`create`].
///
/// Every value here is package content. It must never be logged, persisted or
/// sent through telemetry.
pub mod draft {
    use crate::metadata::{ConsignmentKind, Dispatch, Header, Metadata, SourceSystem};

    /// The `FEJRESZ` values a caller supplies, in the M3 order.
    ///
    /// `TESZT` is written from [`HeaderDraft::test`] and is always present:
    /// one official example omits it, which is unresolved rule M11, but a
    /// writer that reproduced that omission would be choosing to emit a
    /// document that does not validate against its own schema.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct HeaderDraft {
        /// `KRX_VERZIOSZAM`.
        pub version: String,
        /// `FORRASRENDSZER_AZONOSITO` (M4).
        pub source_system: SourceSystem,
        /// `KULDEMENY_AZONOSITO`.
        pub consignment_id: String,
        /// `KULDEMENY_LETREHOZASANAK_IDEJE`, the `xs:dateTime` lexical form,
        /// written verbatim and never interpreted: this crate has no clock.
        pub created_at_text: String,
        /// `KULDEMENY_TIPUS` (M4).
        pub consignment_kind: ConsignmentKind,
        /// `TESZT`.
        pub test: bool,
        /// `VONALKOD`, optional.
        pub barcode: Option<String>,
        /// `KULDEMENY_HIVATKOZASI_AZONOSITO`, optional.
        pub reference_id: Option<String>,
        /// `HIBAKOD`, optional.
        pub error_code: Option<String>,
        /// `KULDEMENY_MEGJEGYZES`, optional.
        pub note: Option<String>,
    }

    impl HeaderDraft {
        /// The header these values describe.
        #[must_use]
        pub fn build(self) -> Header {
            Header {
                version: self.version,
                source_system: self.source_system,
                consignment_id: self.consignment_id,
                created_at_text: self.created_at_text,
                consignment_kind: self.consignment_kind,
                test: self.test,
                test_present: true,
                barcode: self.barcode,
                reference_id: self.reference_id,
                error_code: self.error_code,
                note: self.note,
            }
        }
    }

    /// One `EXPEDIALAS` block carrying no reference yet.
    ///
    /// [`crate::create::package`] derives every `MELLEKLET` and the count from
    /// the attachments it is given. A `declared_attachment_count` supplied here
    /// is therefore an assertion about them, checked and refused with
    /// `create.invalid.reference_mismatch` when it disagrees — never written in
    /// place of the derived count.
    ///
    /// The block carries a `MELLEKLETEK` container whenever a count is
    /// declared, which is the pairing rule M7 describes, and none when the
    /// caller declares nothing and supplies no attachment. Either way
    /// [`crate::create::package`] adds the container as soon as it has a
    /// reference to place in it; a caller that wants the other shape says so
    /// with [`Dispatch::with_attachment_container`].
    #[must_use]
    pub fn dispatch(declared_attachment_count: Option<i64>) -> Dispatch {
        Dispatch {
            declared_attachment_count,
            attachments: Vec::new(),
            attachments_present: declared_attachment_count.is_some(),
            handling_instructions_unqualified: false,
        }
    }

    /// A document carrying `header` and `dispatches`, and no marker element.
    ///
    /// `ERKEZTETES`, `BONTASOK` and `TERTIVEVENY` are absent: the reader
    /// records only their presence and never interprets their content, so the
    /// writer has nothing to put in one.
    #[must_use]
    pub fn metadata(header: Header, dispatches: Vec<Dispatch>) -> Metadata {
        Metadata {
            header,
            receipt_present: false,
            openings_present: false,
            return_receipt_present: false,
            dispatches,
            unknown_elements: 0,
        }
    }
}
