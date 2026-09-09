//! openKRX development foundation.
//!
//! The crate implements three processing layers over caller-supplied bytes:
//! [`archive::inventory`], a bounded, profile-agnostic ZIP reader for hostile
//! input; [`metadata::parse`], bounded schema-shaped parsing of a
//! `KER_META_V0_9` document; and [`profile::check`], a fixed inventory of
//! structural checks over the two. None performs filesystem, clock, process or
//! network access, and none resolves anything external.
//!
//! No extraction, writing or CLI command exists, so [`capabilities`] still
//! reports no operations. Nothing here asserts that an archive is a KRX
//! package: `docs/profile.md` lists essential rules as unresolved, and each of
//! them maps to a distinct [`profile::CheckOutcome::Unresolved`] outcome rather
//! than to a pass or a failure.

#![warn(missing_docs)]

use serde::Serialize;

pub mod archive;
mod error;
mod limits;
pub mod metadata;
pub mod profile;

pub use error::{
    AmbiguityKind, ArchiveError, LimitKind, MalformedKind, Structure, UnsafeNameKind,
    UnsupportedKind,
};
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
/// The operation list stays empty while no command exposes a package
/// operation: a library check is not a package operation, and a structural
/// report is not a conformance verdict.
pub const fn capabilities() -> Capabilities {
    Capabilities {
        project: "openKRX",
        stage: "scaffold",
        operations: &[],
    }
}
