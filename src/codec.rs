use std::io::Read;

use crate::error::{QztError, Result};
use crate::primitives::{u64_to_usize, usize_to_u64};

pub(crate) fn encode_chunk(input: &[u8], level: i32) -> Result<Vec<u8>> {
    zstd::stream::encode_all(input, level).map_err(|_| QztError::ZstdEncodeError)
}

// Exact decoded size and checksums belong to the container's chunk validation.
pub(crate) fn decode_chunk(
    compressed: &[u8],
    dictionary: &[u8],
    expected_size: u64,
) -> Result<Vec<u8>> {
    let decoder = zstd::stream::Decoder::with_dictionary(compressed, dictionary)
        .map_err(|_| QztError::ZstdDecodeError)?;
    let capacity = u64_to_usize(expected_size)?;
    // One extra byte detects overflow without consuming unbounded output.
    let read_limit = expected_size
        .checked_add(1)
        .ok_or(QztError::ResourceLimitExceeded)?;
    let mut decoded = Vec::with_capacity(capacity);
    let mut limited = decoder.take(read_limit);
    limited
        .read_to_end(&mut decoded)
        .map_err(|_| QztError::ZstdDecodeError)?;

    if usize_to_u64(decoded.len())? > expected_size {
        return Err(QztError::ResourceLimitExceeded);
    }

    Ok(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoded_output_at_limit_succeeds_and_over_limit_is_rejected() {
        let compressed = zstd::stream::encode_all(&b"alpha"[..], 0).expect("fixture frame");
        for (limit, expected) in [
            (5, Ok(b"alpha".to_vec())),
            (4, Err(QztError::ResourceLimitExceeded)),
            (0, Err(QztError::ResourceLimitExceeded)),
        ] {
            assert_eq!(decode_chunk(&compressed, &[], limit), expected);
        }
    }
}
