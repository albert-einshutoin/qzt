use std::thread;

use qzt::error::QztError;
use qzt::reader::{QztFileReader, QztReader};
use qzt::schema::{Checksum, DocumentIndex};
use qzt::writer::pack_bytes_with_document_index;
mod support;
use support::{document, writer_options};

#[cfg(windows)]
use qzt::search::SearchOptions;
#[cfg(windows)]
use qzt::sidecar::{QziFileSidecar, SidecarIndexKind, build_search_sidecar};
#[cfg(windows)]
use support::assert_semantic_report_eq;

#[test]
fn verified_range_read_returns_bytes_or_fails_closed() {
    let input = b"alpha\nbeta\ngamma\n";
    let container = qzt::writer::pack_bytes(input, writer_options(8, 8)).expect("pack");
    let reader = QztReader::open(&container).expect("reader");
    let expected = Checksum::blake3(b"beta\n");
    let wrong = Checksum::blake3(b"wrong");

    assert_eq!(
        reader
            .read_range_verified(6, 5, &expected)
            .expect("verified"),
        b"beta\n"
    );
    assert_eq!(
        reader.read_range_verified(6, 5, &wrong),
        Err(QztError::VerifiedChecksumMismatch)
    );
}

#[test]
fn verified_document_read_resolves_document_index() {
    let input = b"doc-one\ndoc-two\n";
    let document_index = DocumentIndex {
        container_id: [0x21; 16],
        documents: vec![
            document(&support::DocumentFixture {
                doc_id: "doc-one",
                input,
                logical_offset: 0,
                byte_length: 8,
                first_line: 0,
                line_count: 1,
                chunk_start: 0,
                chunk_end: 1,
            }),
            document(&support::DocumentFixture {
                doc_id: "doc-two",
                input,
                logical_offset: 8,
                byte_length: 8,
                first_line: 1,
                line_count: 1,
                chunk_start: 1,
                chunk_end: 2,
            }),
        ],
    };
    let container =
        pack_bytes_with_document_index(input, [0x21; 16], writer_options(8, 8), &document_index)
            .expect("pack");
    let memory = QztReader::open(&container).expect("memory reader");
    let file =
        QztFileReader::open_read_at(&container[..], container.len() as u64).expect("file reader");

    assert_eq!(memory.read_document("doc-two").expect("doc"), b"doc-two\n");
    assert_eq!(
        file.read_document_verified("doc-one", &Checksum::blake3(b"doc-one\n"))
            .expect("verified doc"),
        b"doc-one\n"
    );
}

#[test]
fn concurrent_file_backed_verified_reads_match_serial_reads() {
    let input = b"alpha\nbeta\ngamma\ndelta\n";
    let container = qzt::writer::pack_bytes(input, writer_options(8, 8)).expect("pack");
    let reader =
        QztFileReader::open_read_at(&container[..], container.len() as u64).expect("file reader");
    let expected = Checksum::blake3(b"beta\n");
    let serial = reader
        .read_range_verified(6, 5, &expected)
        .expect("serial verified");

    thread::scope(|scope| {
        let first = scope.spawn(|| reader.read_range_verified(6, 5, &expected));
        let second = scope.spawn(|| reader.read_range_verified(6, 5, &expected));
        assert_eq!(first.join().expect("thread"), Ok(serial.clone()));
        assert_eq!(second.join().expect("thread"), Ok(serial));
    });
}

#[cfg(windows)]
#[test]
fn windows_file_backed_range_and_search_match_serial_under_concurrency() {
    use std::sync::Barrier;
    use std::time::{SystemTime, UNIX_EPOCH};

    let input = b"alpha\nbeta needle\ngamma\ndelta needle\n";
    let container = qzt::writer::pack_bytes(input, writer_options(8, 8)).expect("pack");
    let sidecar_bytes =
        build_search_sidecar(&container, SidecarIndexKind::Token).expect("build token sidecar");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let base = std::env::current_dir()
        .expect("test working directory")
        .join("target")
        .join(format!(
            "qzt-windows-concurrency-{}-{nonce}",
            std::process::id()
        ));
    let container_path = base.with_extension("qzt");
    let sidecar_path = base.with_extension("qzi");
    std::fs::write(&container_path, &container).expect("write container fixture");
    std::fs::write(&sidecar_path, &sidecar_bytes).expect("write sidecar fixture");

    let reader = QztFileReader::open_path(&container_path).expect("open file-backed container");
    let sidecar =
        QziFileSidecar::open_path(&sidecar_path, &reader).expect("open file-backed sidecar");
    let range_cases = [
        (6, 12, Checksum::blake3(b"beta needle\n")),
        (24, 13, Checksum::blake3(b"delta needle\n")),
    ];
    let serial_ranges = range_cases.map(|(offset, length, ref expected)| {
        reader
            .read_range_verified(offset, length, expected)
            .expect("serial verified range")
    });
    let queries = ["needle", "alpha"];
    let serial_searches = queries.map(|query| {
        sidecar
            .search(&reader, query, SearchOptions::default())
            .expect("serial search")
    });
    let barrier = Barrier::new(3);

    thread::scope(|scope| {
        let range = scope.spawn(|| {
            barrier.wait();
            for iteration in 0..64 {
                let case = iteration % range_cases.len();
                let (offset, length, expected) = &range_cases[case];
                assert_eq!(
                    reader.read_range_verified(*offset, *length, expected),
                    Ok(serial_ranges[case].clone())
                );
            }
        });
        let search = scope.spawn(|| {
            barrier.wait();
            for iteration in 0..32 {
                let case = iteration % queries.len();
                let concurrent = sidecar
                    .search(&reader, queries[case], SearchOptions::default())
                    .expect("concurrent search");
                assert_semantic_report_eq(
                    &serial_searches[case],
                    &concurrent,
                    "Windows concurrent search",
                );
            }
        });
        barrier.wait();
        range.join().expect("range thread");
        search.join().expect("search thread");
    });

    drop(sidecar);
    drop(reader);
    std::fs::remove_file(sidecar_path).expect("remove sidecar fixture");
    std::fs::remove_file(container_path).expect("remove container fixture");
}
