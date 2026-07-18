use qzt::error::QztError;
use qzt::{
    Checksum, DocumentEntry, DocumentIndex, QztFileReader, QztReader, VerifyLevel, WriterBuilder,
    export_all, pack_bytes_with_container_id,
};
use std::collections::BTreeSet;
mod support;
use support::writer_options;

const CRATE_ROOT: &str = include_str!("../src/lib.rs");

#[test]
fn public_writer_surface_uses_builder_for_optional_features() {
    let writer_exports = CRATE_ROOT
        .split_once("pub use writer::{")
        .and_then(|(_, rest)| rest.split_once("};"))
        .map(|(exports, _)| exports)
        .expect("crate root must export the writer surface");

    let actual = writer_exports
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect::<BTreeSet<_>>();
    let expected = [
        "DocumentSpan",
        "QztFileWriter",
        "WriterBuilder",
        "WriterOptions",
        "export_all",
        "pack_bytes",
        "pack_bytes_with_container_id",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected, "unexpected public writer surface");
}

#[test]
fn writer_builder_reproduces_simplified_pack_entry_points() {
    let input = b"alpha\nbeta\ngamma\n";
    let container_id = [0x20; 16];

    assert_eq!(
        WriterBuilder::new()
            .options(writer_options(8, 8))
            .container_id(container_id)
            .pack(input),
        pack_bytes_with_container_id(input, container_id, writer_options(8, 8))
    );
    let dense = WriterBuilder::new()
        .options(writer_options(8, 8))
        .container_id(container_id)
        .dense_line_index(true)
        .pack(input)
        .expect("dense pack");
    let details = qzt::skeleton::open_skeleton_details(&dense).expect("dense container opens");
    assert!(details.dense_line_index.is_some());
}

// --- Issue #60: pack profile validation regression tests ---

#[test]
fn profile_validation_writer_builder_rejects_unknown_profile() {
    let result = WriterBuilder::new().profile("bogus").pack(b"hello\n");
    assert_eq!(result.unwrap_err(), QztError::MetadataInvalid);
}

#[test]
fn profile_validation_memory_profile_rejects_missing_document_index() {
    let result = WriterBuilder::new()
        .options(writer_options(8, 8))
        .profile("memory")
        .dense_line_index(false)
        .pack(b"hello\n");
    assert_eq!(result.unwrap_err(), QztError::MetadataInvalid);
}

#[test]
fn profile_validation_core_profile_pack_export_round_trips() {
    let input = b"alpha\nbeta\ngamma\n";
    let container = WriterBuilder::new()
        .options(writer_options(8, 8))
        .profile("core")
        .dense_line_index(false)
        .pack(input)
        .expect("core profile should pack");
    assert_eq!(export_all(&container), Ok(input.to_vec()));
}

#[test]
fn crate_root_public_api_snapshot_compiles() {
    let input = b"alpha\nbeta\n";
    let container = WriterBuilder::new()
        .options(writer_options(8, 8))
        .pack(input)
        .expect("pack");
    let memory = QztReader::open(&container).expect("memory reader");
    let file =
        QztFileReader::open_read_at(&container[..], container.len() as u64).expect("file reader");

    assert_eq!(memory.info(), file.info());
    assert!(file.verify(VerifyLevel::Deep).is_ok());
}

#[test]
fn memory_profile_accepts_document_index() {
    let input = b"hello\nworld\n";
    let container_id = [0x20; 16];
    #[allow(clippy::naive_bytecount)]
    let line_count = input.iter().filter(|&&b| b == b'\n').count() as u64;
    let document_index = DocumentIndex {
        container_id,
        documents: vec![DocumentEntry::new(
            "all",
            0,
            input.len() as u64,
            0,
            line_count,
            0,
            2, // ceil(12/8) = 2 chunks with max_chunk_size=8
            Checksum::blake3(input),
        )],
    };
    let result = WriterBuilder::new()
        .options(writer_options(8, 8))
        .container_id(container_id)
        .profile("memory")
        .document_index(document_index)
        .pack(input);
    assert!(
        result.is_ok(),
        "memory profile with DocumentIndex should succeed"
    );
}

#[test]
fn document_index_supplies_default_container_id() {
    let input = b"hello\n";
    let container_id = [0x42; 16];
    let document_index = DocumentIndex {
        container_id,
        documents: vec![DocumentEntry::new(
            "all",
            0,
            input.len() as u64,
            0,
            1,
            0,
            1,
            Checksum::blake3(input),
        )],
    };

    let container = WriterBuilder::new()
        .options(writer_options(8, 8))
        .profile("memory")
        .document_index(document_index)
        .pack(input)
        .expect("document index ID should become the container ID");
    assert_eq!(
        QztReader::open(container)
            .expect("open")
            .info()
            .container_id,
        container_id
    );
}

#[test]
fn document_index_rejects_explicit_container_id_mismatch_during_pack() {
    let input = b"hello\n";
    let document_index = DocumentIndex {
        container_id: [0x42; 16],
        documents: vec![],
    };

    let result = WriterBuilder::new()
        .container_id([0x24; 16])
        .document_index(document_index)
        .pack(input);
    assert_eq!(result.unwrap_err(), QztError::ContainerIdMismatch);
}
