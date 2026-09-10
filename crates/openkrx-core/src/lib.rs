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
//! None of the four performs filesystem, clock, process or network access, and
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

#![warn(missing_docs)]

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
/// protected extraction and deterministic creation are implemented, so the
/// stage is `reader-writer`; a structural report is not a conformance verdict
/// whichever command produced it, and `create` writing a package is not a
/// statement that a receiving service would accept it.
/// `extract` appears here only because every filesystem property
/// `SECURITY.md` requires of it is held by a test on each supported platform,
/// and `create` only because every package it writes is read back by the
/// writer's own structural check before the command reports success.
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
    #[must_use]
    pub fn dispatch(declared_attachment_count: Option<i64>) -> Dispatch {
        Dispatch {
            declared_attachment_count,
            attachments: Vec::new(),
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
