//! QZT reference implementation library.
//!
//! Phase0 intentionally exposes only stable placeholder modules. Later phases
//! replace these placeholders with the binary format implementation.

pub mod error;
pub mod format;
pub mod reader;
pub mod writer;

/// Returns the implementation version advertised by this crate.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
