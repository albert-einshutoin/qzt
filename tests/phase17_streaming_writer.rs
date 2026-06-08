use std::io::Cursor;

use qzt::chunker::ChunkerOptions;
use qzt::error::QztError;
use qzt::reader::QztReader;
use qzt::writer::{pack_bytes, QztFileWriter, WriterOptions};

fn options(target_chunk_size: usize, max_chunk_size: usize) -> WriterOptions {
    WriterOptions {
        chunker: ChunkerOptions {
            target_chunk_size,
            max_chunk_size,
        },
        zstd_level: 0,
    }
}

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
        let options = options(8, 16);
        assert_eq!(
            stream_pack(input, options, input.len()),
            pack_bytes(input, options)
        );
    }
}

#[test]
fn push_fragmentation_does_not_change_output() {
    let input = b"alpha\nbeta\ngamma\ndelta\nepsilon\n";
    let options = options(8, 16);

    assert_eq!(
        stream_pack(input, options, 1),
        stream_pack(input, options, 7)
    );
    assert_eq!(stream_pack(input, options, 3), pack_bytes(input, options));
}

#[test]
fn streamed_container_round_trips_and_finish_is_single_shot() {
    let input = b"alpha\nbeta\ngamma\n";
    let options = options(8, 16);
    let mut writer = QztFileWriter::new(Cursor::new(Vec::new()), options).expect("writer");
    writer.push(input).expect("push");
    writer.finish().expect("finish");
    assert_eq!(writer.finish(), Err(QztError::WriterAlreadyFinished));

    let container = writer.into_inner().into_inner();
    let reader = QztReader::open(&container).expect("container should open");
    assert_eq!(reader.export_all().expect("export"), input);
}

#[test]
fn streaming_writer_rejects_invalid_utf8() {
    let options = options(8, 16);
    let mut writer = QztFileWriter::new(Cursor::new(Vec::new()), options).expect("writer");
    writer.push(&[0xff]).expect("push buffers final chunk");

    assert_eq!(writer.finish(), Err(QztError::InvalidUtf8));
    assert_eq!(writer.push(b"later"), Err(QztError::WriterAlreadyFinished));
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
