use std::cell::Cell;
use std::fs::OpenOptions;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::rc::Rc;

use qzt::error::QztError;
use qzt::reader::{QztFileReader, QztReader, VerifyLevel};
use qzt::writer::{DocumentSpan, QztFileWriter, WriterBuilder, WriterOptions, pack_bytes};
use qzt::{Checksum, DocumentEntry, DocumentIndex};
mod support;

#[test]
fn streaming_writer_is_byte_identical_to_pack_bytes() {
    let fixtures: &[&[u8]] = &[
        b"",
        b"single line",
        b"alpha\nbeta\ngamma\n",
        b"a\r\nb\r\nc\r\n",
        "日本語\nemoji 😀\n".as_bytes(),
        b"abcdefghijklmnopqrstuvwxyz0123456789\n",
    ];

    for input in fixtures {
        let options = support::writer_options(8, 16);
        let streamed = stream_pack(input, options, input.len()).expect("stream pack");
        assert_eq!(streamed, pack_bytes(input, options).expect("memory pack"));
        assert_default_readers(&streamed, input);
    }
}

#[test]
fn push_fragmentation_does_not_change_output() {
    let input = b"alpha\nbeta\ngamma\ndelta\nepsilon\n";
    let options = support::writer_options(8, 16);

    assert_eq!(
        stream_pack(input, options, 1),
        stream_pack(input, options, 7)
    );
    assert_eq!(stream_pack(input, options, 3), pack_bytes(input, options));
}

#[test]
fn streamed_container_round_trips_and_finish_is_single_shot() {
    let input = b"alpha\nbeta\ngamma\n";
    let options = support::writer_options(8, 16);
    let mut writer = QztFileWriter::new(Cursor::new(Vec::new()), options).expect("writer");
    writer.push(input).expect("push");
    writer.finish().expect("finish");
    assert_eq!(writer.finish(), Err(QztError::WriterAlreadyFinished));

    let container = writer.into_inner().into_inner();
    let reader = QztReader::open(&container).expect("container should open");
    assert_eq!(reader.export_all().expect("export"), input);
    assert_default_readers(&container, input);
}

#[test]
fn streaming_writer_rejects_invalid_utf8() {
    let options = support::writer_options(8, 16);
    let mut writer = QztFileWriter::new(Cursor::new(Vec::new()), options).expect("writer");
    writer.push(&[0xff]).expect("push buffers final chunk");

    assert_eq!(writer.finish(), Err(QztError::InvalidUtf8));
    assert_eq!(writer.push(b"later"), Err(QztError::WriterAlreadyFinished));
}

#[test]
fn nonempty_sink_is_rejected_before_header_write() {
    let original = vec![0x5a; 65_536];
    for position in [0, 32_768, 65_536] {
        let mut cursor = Cursor::new(original.clone());
        cursor.set_position(position);
        let result = QztFileWriter::new(&mut cursor, WriterOptions::default());
        assert_eq!(result.map(|_| ()), Err(QztError::NonEmptyWriterSink));
        assert_eq!(
            cursor.get_ref(),
            &original,
            "constructor changed existing bytes"
        );
        assert_eq!(
            cursor.position(),
            position,
            "rejected sink position changed"
        );
    }
}

#[test]
fn empty_sink_with_capacity_and_advanced_position_is_normalized() {
    let mut cursor = Cursor::new(Vec::with_capacity(1024));
    cursor.set_position(512);
    let mut writer = QztFileWriter::new(cursor, WriterOptions::default()).expect("empty sink");
    writer.push(b"hi").expect("push");
    writer.finish().expect("finish");
    let cursor = writer.into_inner();
    assert_eq!(cursor.position(), cursor.get_ref().len() as u64);
    assert_default_readers(cursor.get_ref(), b"hi");
}

#[test]
fn optional_indexes_remain_readable_by_default_readers() {
    let input = "alpha\r\nbeta😀\nlong continuation line\n".as_bytes();
    let container = WriterBuilder::new()
        .options(support::writer_options(8, 12))
        .profile("memory")
        .dense_line_index(true)
        .document_spans(vec![DocumentSpan::new("all", 0, input.len() as u64)])
        .pack(input)
        .expect("optional-index pack");
    assert_default_readers(&container, input);
}

#[test]
fn writer_builder_rejects_chunk_size_above_default_reader_limit() {
    let limit = 64 * 1024 * 1024;
    let options = WriterOptions {
        chunker: qzt::ChunkerOptions {
            target_chunk_size: limit,
            max_chunk_size: limit + 1,
        },
        ..WriterOptions::default()
    };
    assert_eq!(
        WriterBuilder::new().options(options).pack(b"").map(|_| ()),
        Err(QztError::ResourceLimitExceeded)
    );

    let at_limit = WriterOptions {
        chunker: qzt::ChunkerOptions {
            max_chunk_size: limit,
            ..options.chunker
        },
        ..options
    };
    let container = WriterBuilder::new()
        .options(at_limit)
        .pack(b"hello\n")
        .expect("boundary option should pack");
    assert_default_readers(&container, b"hello\n");
}

#[derive(Clone, Copy, Debug)]
enum FailAt {
    InitialSeek,
    Seek,
    HeaderWrite,
    PrefixRead,
    FooterWrite,
    Flush,
    FinalFlush,
    FinalLength,
}

struct FaultSink {
    inner: Cursor<Vec<u8>>,
    at: FailAt,
    initialized: bool,
    header_patched: bool,
    read_after_patch: bool,
    flushes: usize,
    writes: Rc<Cell<usize>>,
}

impl Read for FaultSink {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        if self.header_patched && matches!(self.at, FailAt::PrefixRead) {
            return Err(std::io::Error::other("injected prefix read failure"));
        }
        self.read_after_patch |= self.header_patched;
        self.inner.read(output)
    }
}

impl Write for FaultSink {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let position = self.inner.position();
        if (self.initialized && position == 0 && matches!(self.at, FailAt::HeaderWrite))
            || (self.read_after_patch && matches!(self.at, FailAt::FooterWrite))
        {
            return Err(std::io::Error::other("injected write failure"));
        }
        let written = self.inner.write(bytes)?;
        if position == 0 {
            self.header_patched |= self.initialized;
            self.initialized = true;
        }
        self.writes.set(self.writes.get() + 1);
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.flushes += 1;
        if (self.header_patched && matches!(self.at, FailAt::Flush))
            || (self.flushes == 2 && matches!(self.at, FailAt::FinalFlush))
        {
            return Err(std::io::Error::other("injected flush failure"));
        }
        Ok(())
    }
}

impl Seek for FaultSink {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        if !self.initialized
            && matches!(self.at, FailAt::InitialSeek)
            && matches!(position, SeekFrom::Start(0))
        {
            return Err(std::io::Error::other("injected constructor seek failure"));
        }
        if self.initialized
            && matches!(self.at, FailAt::Seek)
            && matches!(position, SeekFrom::Start(offset) if offset > 0)
        {
            return Err(std::io::Error::other("injected seek failure"));
        }
        if self.read_after_patch
            && matches!(self.at, FailAt::FinalLength)
            && matches!(position, SeekFrom::End(0))
        {
            self.inner.get_mut().push(0);
        }
        self.inner.seek(position)
    }
}

#[test]
fn finish_io_failures_poison_writer_without_later_writes() {
    for at in [
        FailAt::Seek,
        FailAt::HeaderWrite,
        FailAt::PrefixRead,
        FailAt::FooterWrite,
        FailAt::Flush,
        FailAt::FinalFlush,
        FailAt::FinalLength,
    ] {
        let writes = Rc::new(Cell::new(0));
        let sink = FaultSink {
            inner: Cursor::new(Vec::new()),
            at,
            initialized: false,
            header_patched: false,
            read_after_patch: false,
            flushes: 0,
            writes: writes.clone(),
        };
        let mut writer = QztFileWriter::new(sink, WriterOptions::default()).expect("constructor");
        let result = writer.finish();
        if matches!(at, FailAt::FinalLength) {
            assert_eq!(result, Err(QztError::FinalFileSizeMismatch));
        } else {
            assert!(matches!(result, Err(QztError::Io(_))), "{at:?}: {result:?}");
        }
        let before = writes.get();
        assert_eq!(
            writer.push(b"later"),
            Err(QztError::WriterAlreadyFinished),
            "{at:?}"
        );
        assert_eq!(
            writer.finish(),
            Err(QztError::WriterAlreadyFinished),
            "{at:?}"
        );
        assert_eq!(writes.get(), before, "{at:?} wrote after failure");
    }
}

#[test]
fn constructor_seek_failure_writes_nothing_and_push_failure_poisoning() {
    let writes = Rc::new(Cell::new(0));
    let sink = FaultSink {
        inner: Cursor::new(Vec::new()),
        at: FailAt::InitialSeek,
        initialized: false,
        header_patched: false,
        read_after_patch: false,
        flushes: 0,
        writes: writes.clone(),
    };
    assert!(matches!(
        QztFileWriter::new(sink, WriterOptions::default()),
        Err(QztError::Io(_))
    ));
    assert_eq!(writes.get(), 0);

    let sink = FaultSink {
        inner: Cursor::new(Vec::new()),
        at: FailAt::Seek,
        initialized: false,
        header_patched: false,
        read_after_patch: false,
        flushes: 0,
        writes: writes.clone(),
    };
    let mut writer = QztFileWriter::new(sink, support::writer_options(1, 1)).expect("constructor");
    assert!(matches!(writer.push(b"abc"), Err(QztError::Io(_))));
    let before = writes.get();
    assert_eq!(writer.push(b"later"), Err(QztError::WriterAlreadyFinished));
    assert_eq!(writer.finish(), Err(QztError::WriterAlreadyFinished));
    assert_eq!(writes.get(), before);
}

struct BufferedSink {
    committed: Vec<u8>,
    pending: Vec<(u64, Vec<u8>)>,
    position: u64,
    flushes: usize,
}

impl BufferedSink {
    fn end(&self) -> u64 {
        self.pending
            .iter()
            .map(|(start, bytes)| start + bytes.len() as u64)
            .max()
            .unwrap_or(0)
            .max(self.committed.len() as u64)
    }
}

impl Read for BufferedSink {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        let start = usize::try_from(self.position).map_err(std::io::Error::other)?;
        let bytes = self.committed.get(start..).unwrap_or(&[]);
        let count = bytes.len().min(output.len());
        output[..count].copy_from_slice(&bytes[..count]);
        self.position += count as u64;
        Ok(count)
    }
}

impl Write for BufferedSink {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.pending.push((self.position, bytes.to_vec()));
        self.position += bytes.len() as u64;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        for (start, bytes) in self.pending.drain(..) {
            let start = usize::try_from(start).map_err(std::io::Error::other)?;
            let end = start + bytes.len();
            if self.committed.len() < end {
                self.committed.resize(end, 0);
            }
            self.committed[start..end].copy_from_slice(&bytes);
        }
        self.flushes += 1;
        Ok(())
    }
}

impl Seek for BufferedSink {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        let next = match position {
            SeekFrom::Start(offset) => i128::from(offset),
            SeekFrom::Current(offset) => i128::from(self.position) + i128::from(offset),
            SeekFrom::End(offset) => i128::from(self.end()) + i128::from(offset),
        };
        self.position = u64::try_from(next).map_err(|_| std::io::Error::other("bad seek"))?;
        Ok(self.position)
    }
}

#[test]
fn buffered_sink_is_flushed_before_prefix_read_and_success() {
    let sink = BufferedSink {
        committed: Vec::new(),
        pending: Vec::new(),
        position: 99,
        flushes: 0,
    };
    let mut writer = QztFileWriter::new(sink, WriterOptions::default()).expect("empty buffer");
    writer.push(b"buffered\n").expect("push");
    writer.finish().expect("finish");
    let sink = writer.into_inner();
    assert!(sink.pending.is_empty());
    assert!(sink.flushes >= 2);
    assert_eq!(sink.position, sink.committed.len() as u64);
    assert_default_readers(&sink.committed, b"buffered\n");
}

#[test]
fn empty_append_mode_file_cannot_report_unreadable_success() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("append.qzt");
    let file = OpenOptions::new()
        .read(true)
        .append(true)
        .create_new(true)
        .open(&path)
        .expect("append-mode file");
    let mut writer = QztFileWriter::new(file, WriterOptions::default()).expect("empty sink");
    writer.push(b"hello\n").expect("push");
    let result = writer.finish();
    let bytes = std::fs::read(&path).expect("read");
    if result.is_ok() {
        assert!(
            QztReader::open(&bytes).is_err(),
            "reproduce unreadable output"
        );
        panic!("append-mode sink reported success for unreadable output");
    }
}

#[test]
fn writer_rejects_document_index_that_default_reader_cannot_decode() {
    let doc_id = "d".repeat(16 * 1024 * 1024);
    let index = DocumentIndex {
        container_id: [0x29; 16],
        documents: vec![DocumentEntry::new(
            doc_id,
            0,
            0,
            0,
            0,
            0,
            0,
            Checksum::blake3(b""),
        )],
    };
    let result = WriterBuilder::new().document_index(index).pack(b"");
    if let Ok(bytes) = result {
        assert_eq!(
            QztReader::open(&bytes).map(|_| ()),
            Err(QztError::ResourceLimitExceeded),
            "reproduce default Reader allocation limit"
        );
        panic!("Writer reported success for output rejected by the default Reader");
    }
}

fn stream_pack(input: &[u8], options: WriterOptions, step: usize) -> qzt::error::Result<Vec<u8>> {
    let mut writer = QztFileWriter::new(Cursor::new(Vec::new()), options)?;
    if input.is_empty() {
        writer.push(input)?;
    } else {
        for chunk in input.chunks(step.max(1)) {
            writer.push(chunk)?;
        }
    }
    writer.finish()?;
    Ok(writer.into_inner().into_inner())
}

fn assert_default_readers(container: &[u8], input: &[u8]) {
    let memory = QztReader::open(container).expect("memory open");
    let file = QztFileReader::open_read_at(container, container.len() as u64).expect("file open");
    for verified in [
        memory.verify(VerifyLevel::Deep),
        file.verify(VerifyLevel::Deep),
    ] {
        verified.expect("deep verify");
    }
    let mut memory_export = Vec::new();
    let mut file_export = Vec::new();
    memory.export_to(&mut memory_export).expect("memory export");
    file.export_to(&mut file_export).expect("file export");
    assert_eq!(memory_export, input);
    assert_eq!(file_export, input);
}
