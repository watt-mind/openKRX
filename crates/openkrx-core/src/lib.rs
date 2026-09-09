//! openKRX development foundation.
//!
//! The crate implements one processing layer: [`archive::inventory`], a bounded,
//! profile-agnostic ZIP reader for hostile input. It performs no filesystem,
//! clock, process or network access; a caller supplies the whole archive image
//! as bytes. No metadata parsing, extraction, writing or profile validation
//! exists, so [`capabilities`] still reports no operations: an inventory is not
//! a package operation and never asserts that an archive is a KRX package.

#![warn(missing_docs)]

use serde::Serialize;

pub mod archive;
mod error;
mod limits;

pub use error::{
    AmbiguityKind, ArchiveError, LimitKind, MalformedKind, Structure, UnsafeNameKind,
    UnsupportedKind,
};
pub use limits::Limits;

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
pub const fn capabilities() -> Capabilities {
    Capabilities {
        project: "openKRX",
        stage: "scaffold",
        operations: &[],
    }
}
