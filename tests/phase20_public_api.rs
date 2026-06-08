use qzt::{
    pack_bytes_with_container_id, pack_bytes_with_dense_line_index, ChunkerOptions, QztFileReader,
    QztReader, VerifyLevel, WriterBuilder, WriterOptions,
};

fn options() -> WriterOptions {
    WriterOptions {
        chunker: ChunkerOptions {
            target_chunk_size: 8,
            max_chunk_size: 8,
        },
        zstd_level: 0,
    }
}

#[test]
fn writer_builder_reproduces_legacy_pack_entry_points() {
    let input = b"alpha\nbeta\ngamma\n";
    let container_id = [0x20; 16];

    assert_eq!(
        WriterBuilder::new()
            .options(options())
            .container_id(container_id)
            .pack(input),
        pack_bytes_with_container_id(input, container_id, options())
    );
    assert_eq!(
        WriterBuilder::new()
            .options(options())
            .container_id(container_id)
            .dense_line_index(true)
            .pack(input),
        pack_bytes_with_dense_line_index(input, container_id, options())
    );
}

#[test]
fn crate_root_public_api_snapshot_compiles() {
    let input = b"alpha\nbeta\n";
    let container = WriterBuilder::new()
        .options(options())
        .pack(input)
        .expect("pack");
    let memory = QztReader::open(&container).expect("memory reader");
    let file =
        QztFileReader::open_read_at(&container[..], container.len() as u64).expect("file reader");

    assert_eq!(memory.info(), file.info());
    assert!(file.verify(VerifyLevel::Deep).is_ok());
}
