/// QZT v0.1 fixed header magic.
pub const MAGIC: [u8; 8] = *b"QZT\0TXT1";

/// QZT v0.1 footer trailer magic.
pub const TRAILER_MAGIC: [u8; 8] = *b"QZTTAIL1";

/// QZT v0.1 major version.
pub const MAJOR_VERSION: u16 = 0;

/// QZT v0.1 minor version.
pub const MINOR_VERSION: u16 = 1;

/// Fixed Header size in bytes.
pub const HEADER_LEN: usize = 128;

/// Fixed Footer Trailer size in bytes.
pub const FOOTER_TRAILER_LEN: usize = 64;
