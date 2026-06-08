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
