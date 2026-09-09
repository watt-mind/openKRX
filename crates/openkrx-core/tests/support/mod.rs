//! The crate's test-only synthetic writers, under their historical name.
//!
//! The writers themselves live in `openkrx_core::synthetic`, behind the
//! non-default `synthetic-writer` feature, so that the command-line crate's
//! subprocess tests can build the same archives. This module re-exports them
//! so each test file keeps reading `support::…`.

pub use openkrx_core::synthetic::*;
