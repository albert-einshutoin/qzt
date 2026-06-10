//! QZT reference implementation library.
//!
//! Use the re-exported reader, writer, error, checksum, and validation corpus
//! types as the stable v0.1 technical-preview surface.

#![cfg_attr(not(feature = "internal-testing"), warn(missing_docs))]

#[cfg(feature = "internal-testing")]
#[doc(hidden)]
pub mod benchmark;
#[cfg(not(feature = "internal-testing"))]
mod benchmark;
#[cfg(feature = "internal-testing")]
#[doc(hidden)]
pub mod cbor;
// Some module items exist only for the internal-testing surface; they are
// unreachable (and therefore dead code) in the curated default build.
#[cfg(not(feature = "internal-testing"))]
#[allow(dead_code)]
mod cbor;
#[cfg(feature = "internal-testing")]
#[doc(hidden)]
pub mod chunk_table;
#[cfg(not(feature = "internal-testing"))]
mod chunk_table;
#[cfg(feature = "internal-testing")]
#[doc(hidden)]
pub mod chunker;
#[cfg(not(feature = "internal-testing"))]
mod chunker;
#[cfg(feature = "internal-testing")]
#[doc(hidden)]
pub mod corpus;
#[cfg(not(feature = "internal-testing"))]
mod corpus;
#[cfg(feature = "internal-testing")]
#[doc(hidden)]
pub mod dense_line_index;
#[cfg(not(feature = "internal-testing"))]
mod dense_line_index;
#[cfg(feature = "internal-testing")]
#[doc(hidden)]
pub mod error;
#[cfg(not(feature = "internal-testing"))]
mod error;
#[cfg(feature = "internal-testing")]
#[doc(hidden)]
pub mod fixed;
#[cfg(not(feature = "internal-testing"))]
mod fixed;
#[cfg(feature = "internal-testing")]
#[doc(hidden)]
pub mod format;
#[cfg(not(feature = "internal-testing"))]
#[allow(dead_code)]
mod format;
#[cfg(feature = "internal-testing")]
#[doc(hidden)]
pub mod io;
#[cfg(not(feature = "internal-testing"))]
mod io;
#[cfg(feature = "internal-testing")]
#[doc(hidden)]
pub mod limits;
#[cfg(not(feature = "internal-testing"))]
mod limits;
#[cfg(feature = "internal-testing")]
#[doc(hidden)]
pub mod primitives;
#[cfg(not(feature = "internal-testing"))]
mod primitives;
#[cfg(feature = "internal-testing")]
#[doc(hidden)]
pub mod reader;
#[cfg(not(feature = "internal-testing"))]
mod reader;
#[cfg(feature = "internal-testing")]
#[doc(hidden)]
pub mod schema;
#[cfg(not(feature = "internal-testing"))]
#[allow(dead_code)]
mod schema;
#[cfg(feature = "internal-testing")]
#[doc(hidden)]
pub mod search;
#[cfg(not(feature = "internal-testing"))]
mod search;
#[cfg(feature = "internal-testing")]
#[doc(hidden)]
pub mod sidecar;
#[cfg(not(feature = "internal-testing"))]
mod sidecar;
#[cfg(feature = "internal-testing")]
#[doc(hidden)]
pub mod skeleton;
#[cfg(not(feature = "internal-testing"))]
#[allow(dead_code)]
mod skeleton;
#[cfg(feature = "internal-testing")]
#[doc(hidden)]
pub mod writer;
#[cfg(not(feature = "internal-testing"))]
#[allow(dead_code)]
mod writer;

pub use benchmark::{
    run_competitive_benchmark, run_release_benchmark, CompetitiveBenchmarkOptions,
    CompetitiveBenchmarkReport, ReleaseBenchmarkOptions, ReleaseBenchmarkReport,
};
pub use chunker::ChunkerOptions;
pub use corpus::{generate_validation_corpus, CorpusKind, ValidationCorpusOptions};
pub use error::{QztError, Result};
pub use io::ReadAt;
pub use limits::ResourceLimits;
pub use reader::{QztFileReader, QztInfo, QztReader, VerifyLevel, VerifyReport};
pub use schema::{Checksum, DocumentEntry, DocumentIndex};
pub use search::{
    NgramIndexBuildOptions, RawNgramIndex, RawTokenIndex, SearchIndexSource, SearchOptions,
    TokenIndexBuildOptions,
};
pub use sidecar::{
    build_search_sidecar, build_search_sidecar_from_file, QziFileSidecar, QziSidecar,
    SidecarIndexKind,
};
#[doc(hidden)]
pub use skeleton::open_skeleton_details;
pub use writer::{
    export_all, pack_bytes, pack_bytes_with_container_id, pack_bytes_with_dense_line_index,
    pack_bytes_with_document_index, pack_bytes_with_memory_profile, pack_bytes_with_profile,
    QztFileWriter, WriterBuilder, WriterOptions,
};

/// Returns the implementation version advertised by this crate.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
