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
//! these layers, and [`capabilities`] names them beside `extract`, the one
//! command that writes: `openkrx-cli` joins [`extract::plan`] onto a
//! caller-selected destination under a no-clobber, no-link, undo-on-failure
//! policy. No command creates a package: [`create::package`], the library's
//! deterministic writer for the canonical documented layout, exists, but no
//! command exposes it yet and [`capabilities`] therefore does not name it.
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
/// The list names the package operations a command actually performs. Reading
/// and protected extraction are implemented; creation is a library function
/// ([`create::package`]) that no command exposes yet, so it is deliberately not
/// listed here, and a structural report is not a conformance verdict whichever
/// command produced it.
/// `extract` appears here only because every filesystem property
/// `SECURITY.md` requires of it is held by a test on each supported platform.
pub const fn capabilities() -> Capabilities {
    Capabilities {
        project: "openKRX",
        stage: "reader",
        operations: &["inspect", "list", "validate-structure", "extract"],
    }
}
