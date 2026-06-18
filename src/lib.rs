//! QZT reference implementation library.
//!
//! Use the re-exported reader, writer, error, checksum, and validation corpus
//! types as the stable v0.1 technical-preview surface.

#![cfg_attr(not(feature = "internal-testing"), warn(missing_docs))]

// Some module items exist only for the internal-testing surface; they are
// unreachable (and therefore dead code) in the curated default build.
/// Declares a module that is private by default but exported (hidden) under
/// the `internal-testing` feature so integration tests can reach internals.
macro_rules! internal_module {
    ($(#[$meta:meta])* $name:ident) => {
        #[cfg(feature = "internal-testing")]
        #[doc(hidden)]
        pub mod $name;
        #[cfg(not(feature = "internal-testing"))]
        $(#[$meta])*
        mod $name;
    };
}

internal_module!(benchmark);
internal_module!(
    #[allow(dead_code)]
    cbor
);
internal_module!(chunk_table);
internal_module!(chunker);
internal_module!(corpus);
internal_module!(dense_line_index);
internal_module!(error);
internal_module!(fixed);
internal_module!(
    #[allow(dead_code)]
    format
);
internal_module!(io);
internal_module!(limits);
internal_module!(primitives);
internal_module!(reader);
internal_module!(
    #[allow(dead_code)]
    schema
);
internal_module!(search);
internal_module!(sidecar);
internal_module!(
    #[allow(dead_code)]
    skeleton
);
internal_module!(
    #[allow(dead_code)]
    writer
);

pub use benchmark::{
    CompetitiveBenchmarkOptions, CompetitiveBenchmarkReport, ReleaseBenchmarkOptions,
    ReleaseBenchmarkReport, run_competitive_benchmark, run_release_benchmark,
};
pub use chunker::ChunkerOptions;
pub use corpus::{CorpusKind, ValidationCorpusOptions, generate_validation_corpus};
pub use error::{QztError, Result};
pub use io::ReadAt;
pub use limits::ResourceLimits;
pub use reader::{QztFileReader, QztInfo, QztReader, VerifyLevel, VerifyReport};
pub use schema::{Checksum, DocumentEntry, DocumentIndex};
pub use search::{
    NgramIndexBuildOptions, PlannerDecision, RawNgramIndex, RawTokenIndex, SearchHit,
    SearchIndexSource, SearchMetrics, SearchOptions, SearchReport, TokenIndexBuildOptions,
};
pub use sidecar::{
    QziFileSidecar, QziSidecar, SidecarIndexKind, build_search_sidecar,
    build_search_sidecar_from_file,
};
#[doc(hidden)]
pub use skeleton::open_skeleton_details;
pub use writer::{
    QztFileWriter, WriterBuilder, WriterOptions, export_all, pack_bytes,
    pack_bytes_with_container_id, pack_bytes_with_dense_line_index, pack_bytes_with_document_index,
    pack_bytes_with_memory_profile, pack_bytes_with_profile,
};

/// Returns the implementation version advertised by this crate.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
