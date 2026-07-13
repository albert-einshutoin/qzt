use std::io::ErrorKind;

use qzt::error::QztError;
use qzt::writer::pack_bytes_with_profile;
use qzt::{
    Checksum, DocumentEntry, DocumentIndex, QztFileReader, QztReader, VerifyLevel, WriterBuilder,
    export_all, pack_bytes_with_container_id, pack_bytes_with_dense_line_index,
};
mod support;
use support::writer_options;
fn assert_human_readable_display(
    error: QztError,
    raw_variant_name: &str,
    human_readable_substrings: &[&str],
) {
    let display = format!("{error}");
    let debug = format!("{error:?}");
    assert_ne!(
        display, debug,
        "Display for {raw_variant_name} should not delegate to Debug"
    );
    assert!(
        !display.contains(raw_variant_name),
        "Display for {raw_variant_name} should not leak raw variant name, got: {display}"
    );
    for substring in human_readable_substrings {
        assert!(
            display.contains(substring),
            "Display for {raw_variant_name} should contain {substring:?}, got: {display}"
        );
    }
}

/// Issue #179: `QztError::Display` must stay human-readable, not raw Debug variant names.
#[test]
fn qzt_error_display_uses_human_readable_messages_for_representative_variants() {
    assert_human_readable_display(
        QztError::InvalidMagic,
        "InvalidMagic",
        &["invalid magic", "QZT container"],
    );
    assert_human_readable_display(
        QztError::InvalidHeader,
        "InvalidHeader",
        &["header", "malformed"],
    );
    assert_human_readable_display(
        QztError::UnsupportedVersion,
        "UnsupportedVersion",
        &["unsupported", "format version"],
    );
    assert_human_readable_display(
        QztError::ContainerCorrupt,
        "ContainerCorrupt",
        &["container", "corrupt"],
    );
    assert_human_readable_display(
        QztError::ResourceLimitExceeded,
        "ResourceLimitExceeded",
        &["resource limit"],
    );
    assert_human_readable_display(
        QztError::DocumentNotFound,
        "DocumentNotFound",
        &["document", "not found"],
    );
    assert_human_readable_display(
        QztError::UnsupportedIndexMode("normalized_utf8 token index"),
        "UnsupportedIndexMode",
        &["index mode", "not supported"],
    );

    let kind = ErrorKind::NotFound;
    let error = QztError::Io(kind);
    let display = format!("{error}");
    let debug = format!("{error:?}");
    assert_ne!(display, debug, "Io Display should not delegate to Debug");
    assert!(
        !display.contains("Io("),
        "Io Display should not use Debug formatting, got: {display}"
    );
    assert!(
        !display.contains("NotFound"),
        "Io Display should not leak ErrorKind variant name, got: {display}"
    );
    assert!(
        display.contains("I/O error:"),
        "Io Display should name the error class, got: {display}"
    );
    assert!(
        display.contains(&kind.to_string()),
        "Io Display should include the OS/Rust error kind text, got: {display}"
    );
}

#[test]
fn writer_builder_reproduces_legacy_pack_entry_points() {
    let input = b"alpha\nbeta\ngamma\n";
    let container_id = [0x20; 16];

    assert_eq!(
        WriterBuilder::new()
            .options(writer_options(8, 8))
            .container_id(container_id)
            .pack(input),
        pack_bytes_with_container_id(input, container_id, writer_options(8, 8))
    );
    assert_eq!(
        WriterBuilder::new()
            .options(writer_options(8, 8))
            .container_id(container_id)
            .dense_line_index(true)
            .pack(input),
        pack_bytes_with_dense_line_index(input, container_id, writer_options(8, 8))
    );
}

// --- Issue #60: pack profile validation regression tests ---

#[test]
fn profile_validation_writer_builder_rejects_unknown_profile() {
    let result = WriterBuilder::new().profile("bogus").pack(b"hello\n");
    assert_eq!(result.unwrap_err(), QztError::MetadataInvalid);
}

#[test]
fn profile_validation_memory_profile_rejects_missing_document_index() {
    let result = pack_bytes_with_profile(b"hello\n", writer_options(8, 8), "memory", false);
    assert_eq!(result.unwrap_err(), QztError::MetadataInvalid);
}

#[test]
fn profile_validation_core_profile_pack_export_round_trips() {
    let input = b"alpha\nbeta\ngamma\n";
    let container = pack_bytes_with_profile(input, writer_options(8, 8), "core", false)
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
