use std::fs::File;
use std::io;

/// Positioned read abstraction used by file-backed QZT readers.
pub trait ReadAt {
    /// Reads exactly `buf.len()` bytes starting at `offset`.
    fn read_exact_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()>;
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
        file.seek(SeekFrom::Start(offset))?;
        let read_result = file.read_exact(buf);
        let restore_result = file.seek(SeekFrom::Start(original));

        match (read_result, restore_result) {
            (Err(error), _) | (Ok(()), Err(error)) => Err(error),
            (Ok(()), Ok(_)) => Ok(()),
        }
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
            4,
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
            4,
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
        file.seek(SeekFrom::Start(4)).expect("set sentinel cursor");
        (path, Mutex::new(file))
    }
}
