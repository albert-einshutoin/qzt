use std::fs::File;
use std::io;

/// Positioned read abstraction used by file-backed QZT readers.
pub trait ReadAt {
    /// Reads exactly `buf.len()` bytes starting at `offset`.
    fn read_exact_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()>;
}

pub(crate) fn hash_read_at_range<R: ReadAt + ?Sized>(
    source: &R,
    offset: u64,
    size: u64,
) -> io::Result<blake3::Hasher> {
    const BUFFER_SIZE: usize = 64 * 1024;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0_u8; BUFFER_SIZE];
    let mut remaining = size;
    let mut position = offset;

    while remaining > 0 {
        let read_len = usize::try_from(remaining.min(BUFFER_SIZE as u64))
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "range too large"))?;
        source.read_exact_at(position, &mut buffer[..read_len])?;
        hasher.update(&buffer[..read_len]);
        position = position
            .checked_add(read_len as u64)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "range overflow"))?;
        remaining -= read_len as u64;
    }

    Ok(hasher)
}

impl ReadAt for &[u8] {
    fn read_exact_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        let start = usize::try_from(offset)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "offset too large"))?;
        let end = start
            .checked_add(buf.len())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "range overflow"))?;
        let source = self
            .get(start..end)
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "short read"))?;
        buf.copy_from_slice(source);
        Ok(())
    }
}

impl ReadAt for Vec<u8> {
    fn read_exact_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        self.as_slice().read_exact_at(offset, buf)
    }
}

#[cfg(unix)]
impl ReadAt for File {
    fn read_exact_at(&self, offset: u64, mut buf: &mut [u8]) -> io::Result<()> {
        use std::os::unix::fs::FileExt;

        let mut current = offset;
        while !buf.is_empty() {
            let read = FileExt::read_at(self, buf, current)?;
            if read == 0 {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "short read"));
            }
            current = current
                .checked_add(read as u64)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "range overflow"))?;
            let (_, rest) = buf.split_at_mut(read);
            buf = rest;
        }
        Ok(())
    }
}

#[cfg(windows)]
impl ReadAt for std::sync::Mutex<File> {
    fn read_exact_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        use std::io::{Read, Seek, SeekFrom};

        // Rust's Windows `seek_read` changes the shared file cursor. Serialize
        // seek/read/restore as one critical section so concurrent partial
        // decompression and search cannot observe or race that cursor.
        let mut file = self
            .lock()
            .map_err(|_| io::Error::other("positioned file read lock poisoned"))?;
        let original = file.stream_position()?;
        if let Err(error) = file.seek(SeekFrom::Start(offset)) {
            return finish_windows_positioned_read(Err(error), file.seek(SeekFrom::Start(original)));
        }
        let read_result = file.read_exact(buf);
        let restore_result = file.seek(SeekFrom::Start(original));
        finish_windows_positioned_read(read_result, restore_result)
    }
}

#[cfg(windows)]
fn finish_windows_positioned_read(
    operation: io::Result<()>,
    restore: io::Result<u64>,
) -> io::Result<()> {
    match (operation, restore) {
        (Ok(()), Ok(_)) => Ok(()),
        (Err(error), Ok(_)) | (Ok(()), Err(error)) => Err(error),
        (Err(operation_error), Err(restore_error)) => Err(io::Error::new(
            restore_error.kind(),
            format!(
                "positioned read failed ({operation_error}); cursor restore also failed ({restore_error})"
            ),
        )),
    }
}

#[cfg(test)]
mod shared_tests {
    use super::*;

    #[test]
    fn range_hash_matches_the_exact_selected_bytes_across_buffer_boundaries() {
        let bytes = (0..(70 * 1024 + 17))
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>();
        let offset = 113_u64;
        let size = 66 * 1024 + 7_usize;

        let actual = hash_read_at_range(&bytes, offset, size as u64)
            .expect("in-memory positioned hash should succeed")
            .finalize();
        let start = usize::try_from(offset).unwrap();
        let expected = blake3::hash(&bytes[start..start + size]);

        assert_eq!(actual, expected);
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::ReadAt;
    use std::fs::{OpenOptions, remove_file};
    use std::io::{Seek, SeekFrom, Write};
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn windows_positioned_read_preserves_cursor() {
        let (path, source) = fixture();

        let mut bytes = [0_u8; 3];
        source
            .read_exact_at(1, &mut bytes)
            .expect("positioned read");
        assert_eq!(&bytes, b"bcd");
        assert_eq!(
            source.lock().expect("lock fixture").stream_position().unwrap(),
            5,
            "positioned reads must not change the observable file cursor"
        );

        drop(source);
        remove_file(path).expect("remove positioned-read fixture");
    }

    #[test]
    fn windows_positioned_read_reports_unexpected_eof() {
        let (path, source) = fixture();
        let error = source
            .read_exact_at(4, &mut [0_u8; 3])
            .expect_err("exact read beyond EOF must fail");
        assert_eq!(error.kind(), std::io::ErrorKind::UnexpectedEof);
        assert_eq!(
            source.lock().expect("lock fixture").stream_position().unwrap(),
            5,
            "failed positioned reads must restore the observable file cursor"
        );

        drop(source);
        remove_file(path).expect("remove positioned-read fixture");
    }

    fn fixture() -> (PathBuf, Mutex<std::fs::File>) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let path = std::env::current_dir()
            .expect("test working directory")
            .join("target")
            .join(format!("qzt-read-at-{}-{nonce}", std::process::id()));
        let mut file = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&path)
            .expect("create positioned-read fixture");
        file.write_all(b"abcdef").expect("write fixture");
        // Keep the sentinel distinct from the successful read end (1 + 3 = 4)
        // so the cursor test fails if restoration is accidentally removed.
        file.seek(SeekFrom::Start(5)).expect("set sentinel cursor");
        (path, Mutex::new(file))
    }
}
