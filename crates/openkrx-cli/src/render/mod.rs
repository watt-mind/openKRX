//! Turning a report into the two output forms, and nothing else.
//!
//! JSON mode writes exactly one object on stdout and leaves stderr empty on
//! success. Human mode writes text on stdout and, on failure, one line on
//! stderr. Neither form ever prints the input path, and only `inspect` prints
//! a declared metadata value, on stdout, because that is what it was asked
//! for. A diagnostic carries a stable code, an entry index and numbers.

pub mod human;
pub mod json;
