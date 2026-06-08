use std::io;
use std::sync::{Arc, Mutex};

use qzt::chunker::ChunkerOptions;
use qzt::io::ReadAt;
use qzt::reader::{QztFileReader, QztReader};
use qzt::skeleton::open_skeleton_details;
use qzt::writer::{pack_bytes_with_container_id, WriterOptions};

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
fn file_reader_open_does_not_touch_chunk_data_region() {
    let input = b"alpha\nbeta\ngamma\ndelta\n";
    let container =
        pack_bytes_with_container_id(input, [0x15; 16], options()).expect("pack should work");
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
    let container =
        pack_bytes_with_container_id(input, [0x16; 16], options()).expect("pack should work");
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
    let container =
        pack_bytes_with_container_id(input, [0x17; 16], options()).expect("pack should work");
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

#[derive(Clone)]
struct CountingReadAt {
    bytes: Arc<Vec<u8>>,
    reads: Arc<Mutex<Vec<(u64, u64)>>>,
}

impl CountingReadAt {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes: Arc::new(bytes),
            reads: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl ReadAt for CountingReadAt {
    fn read_exact_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        self.reads
            .lock()
            .map_err(|_| io::Error::other("poisoned reads lock"))?
            .push((offset, buf.len() as u64));
        let start = usize::try_from(offset)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "offset too large"))?;
        let end = start
            .checked_add(buf.len())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "range overflow"))?;
        let source = self
            .bytes
            .get(start..end)
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "short read"))?;
        buf.copy_from_slice(source);
        Ok(())
    }
}

fn overlaps(left_offset: u64, left_size: u64, right_offset: u64, right_size: u64) -> bool {
    let left_end = left_offset.saturating_add(left_size);
    let right_end = right_offset.saturating_add(right_size);
    left_offset < right_end && right_offset < left_end
}
