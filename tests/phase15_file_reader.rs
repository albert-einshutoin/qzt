use qzt::reader::{QztFileReader, QztReader};
use qzt::skeleton::open_skeleton_details;
use qzt::writer::pack_bytes_with_container_id;
mod support;
use support::{CountingReadAt, writer_options};

#[test]
fn file_reader_open_does_not_touch_chunk_data_region() {
    let input = b"alpha\nbeta\ngamma\ndelta\n";
    let container = pack_bytes_with_container_id(input, [0x15; 16], writer_options(8, 8))
        .expect("pack should work");
    let details = open_skeleton_details(&container).expect("container should open");
    let counting = CountingReadAt::new(container.clone());
    let reads = counting.reads.clone();

    let reader =
        QztFileReader::open_read_at(counting, container.len() as u64).expect("open should work");

    assert_eq!(reader.info().original_size, input.len() as u64);
    let open_reads = reads.lock().expect("reads lock").clone();
    for (offset, size) in open_reads {
        for entry in &details.chunk_entries {
            assert!(
                !overlaps(offset, size, entry.physical_offset, entry.compressed_size),
                "open read {offset}:{size} overlapped chunk {}",
                entry.chunk_id
            );
        }
    }
}

#[test]
fn file_reader_matches_in_memory_reader_for_range_line_and_export() {
    let input = "alpha\nβeta line\ngamma\nlast line".as_bytes();
    let container = pack_bytes_with_container_id(input, [0x16; 16], writer_options(8, 8))
        .expect("pack should work");
    let memory = QztReader::open(&container).expect("memory reader should open");
    let file = QztFileReader::open_read_at(&container[..], container.len() as u64)
        .expect("file reader should open");

    assert_eq!(file.info(), memory.info());
    assert_eq!(file.read_range(2, 17), memory.read_range(2, 17));
    assert_eq!(file.read_line_raw(1), memory.read_line_raw(1));

    let mut exported = Vec::new();
    file.export_to(&mut exported).expect("export should work");
    assert_eq!(exported, input);
}

#[test]
fn file_reader_range_reads_only_overlapping_chunks() {
    let input = b"aaaaaaa\nbbbbbbb\nccccccc\nddddddd\n";
    let container = pack_bytes_with_container_id(input, [0x17; 16], writer_options(8, 8))
        .expect("pack should work");
    let details = open_skeleton_details(&container).expect("container should open");
    let counting = CountingReadAt::new(container.clone());
    let reads = counting.reads.clone();
    let reader =
        QztFileReader::open_read_at(counting, container.len() as u64).expect("open should work");
    reads.lock().expect("reads lock").clear();

    assert_eq!(
        reader.read_range(9, 10).expect("range should read"),
        b"bbbbbb\nccc"
    );

    let range_reads = reads.lock().expect("reads lock").clone();
    let chunk_reads = range_reads
        .iter()
        .filter(|(offset, size)| {
            details
                .chunk_entries
                .iter()
                .any(|entry| overlaps(*offset, *size, entry.physical_offset, entry.compressed_size))
        })
        .count();
    assert_eq!(chunk_reads, 2);
}

#[test]
fn open_path_reports_io_error_for_missing_file() {
    assert_eq!(
        QztFileReader::open_path("/nonexistent/qzt-test-missing.qzt").map(|_| ()),
        Err(qzt::error::QztError::Io(std::io::ErrorKind::NotFound))
    );
}

fn overlaps(left_offset: u64, left_size: u64, right_offset: u64, right_size: u64) -> bool {
    let left_end = left_offset.saturating_add(left_size);
    let right_end = right_offset.saturating_add(right_size);
    left_offset < right_end && right_offset < left_end
}
