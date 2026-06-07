use crate::error::{QztError, Result};

const MAX_PHASE1_ALLOCATION: u64 = 16 * 1024 * 1024;
const MAX_PHASE1_ITEMS: u64 = 1_000_000;

/// Small CBOR value model for deterministic validation and schema checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CborValue {
    Integer(i128),
    Bytes(Vec<u8>),
    Text(String),
    Array(Vec<CborValue>),
    Map(Vec<(CborValue, CborValue)>),
    Bool(bool),
    Null,
}

/// Closed-schema rules for text-keyed CBOR maps.
#[derive(Debug, Clone, Copy)]
pub struct TextKeySchema<'a> {
    pub required: &'a [&'a str],
    pub optional: &'a [&'a str],
    pub allow_unknown: bool,
}

/// Validates a complete QZT deterministic CBOR item.
pub fn validate_deterministic(input: &[u8]) -> Result<CborValue> {
    let mut parser = Parser { input, offset: 0 };
    let value = parser.parse_value()?;

    if parser.offset != input.len() {
        return Err(QztError::NonCanonicalCbor);
    }

    Ok(value)
}

/// Encodes a CBOR value using the QZT deterministic profile.
pub fn encode_deterministic(value: &CborValue) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    encode_value(value, &mut out)?;
    Ok(out)
}

/// Validates a deterministic text-keyed map against a closed schema.
pub fn validate_text_key_schema(input: &[u8], schema: TextKeySchema<'_>) -> Result<CborValue> {
    let value = validate_deterministic(input)?;
    let CborValue::Map(entries) = &value else {
        return Err(QztError::MetadataInvalid);
    };

    for required in schema.required {
        let exists = entries
            .iter()
            .any(|(key, _)| key == &CborValue::Text((*required).to_owned()));
        if !exists {
            return Err(QztError::MetadataInvalid);
        }
    }

    for (key, _) in entries {
        let CborValue::Text(key) = key else {
            return Err(QztError::MetadataInvalid);
        };

        let known =
            schema.required.contains(&key.as_str()) || schema.optional.contains(&key.as_str());

        if !known && !schema.allow_unknown {
            return Err(QztError::MetadataInvalid);
        }
    }

    Ok(value)
}

fn encode_value(value: &CborValue, out: &mut Vec<u8>) -> Result<()> {
    match value {
        CborValue::Integer(value) if *value >= 0 => {
            let value = u64::try_from(*value).map_err(|_| QztError::ResourceLimitExceeded)?;
            encode_type_and_argument(0, value, out);
        }
        CborValue::Integer(value) => {
            let magnitude = value
                .checked_add(1)
                .and_then(i128::checked_neg)
                .and_then(|value| u64::try_from(value).ok())
                .ok_or(QztError::ResourceLimitExceeded)?;
            encode_type_and_argument(1, magnitude, out);
        }
        CborValue::Bytes(bytes) => {
            encode_type_and_argument(2, len_to_u64(bytes.len())?, out);
            out.extend_from_slice(bytes);
        }
        CborValue::Text(text) => {
            encode_type_and_argument(3, len_to_u64(text.len())?, out);
            out.extend_from_slice(text.as_bytes());
        }
        CborValue::Array(values) => {
            encode_type_and_argument(4, len_to_u64(values.len())?, out);
            for value in values {
                encode_value(value, out)?;
            }
        }
        CborValue::Map(entries) => {
            encode_type_and_argument(5, len_to_u64(entries.len())?, out);
            let mut encoded_entries = Vec::with_capacity(entries.len());

            for (key, value) in entries {
                let key_bytes = encode_deterministic(key)?;
                let value_bytes = encode_deterministic(value)?;
                encoded_entries.push((key_bytes, value_bytes));
            }

            encoded_entries.sort_by(|left, right| left.0.cmp(&right.0));

            for pair in encoded_entries.windows(2) {
                if pair[0].0 == pair[1].0 {
                    return Err(QztError::DuplicateCborKey);
                }
            }

            for (key, value) in encoded_entries {
                out.extend_from_slice(&key);
                out.extend_from_slice(&value);
            }
        }
        CborValue::Bool(false) => out.push(0xf4),
        CborValue::Bool(true) => out.push(0xf5),
        CborValue::Null => out.push(0xf6),
    }

    Ok(())
}

fn encode_type_and_argument(major: u8, value: u64, out: &mut Vec<u8>) {
    let prefix = major << 5;
    match value {
        0..=23 => out.push(prefix | value as u8),
        24..=0xff => {
            out.push(prefix | 24);
            out.push(value as u8);
        }
        0x100..=0xffff => {
            out.push(prefix | 25);
            out.extend_from_slice(&(value as u16).to_be_bytes());
        }
        0x1_0000..=0xffff_ffff => {
            out.push(prefix | 26);
            out.extend_from_slice(&(value as u32).to_be_bytes());
        }
        _ => {
            out.push(prefix | 27);
            out.extend_from_slice(&value.to_be_bytes());
        }
    }
}

fn len_to_u64(len: usize) -> Result<u64> {
    u64::try_from(len).map_err(|_| QztError::ResourceLimitExceeded)
}

struct Parser<'a> {
    input: &'a [u8],
    offset: usize,
}

impl Parser<'_> {
    fn parse_value(&mut self) -> Result<CborValue> {
        let initial = self.read_u8()?;
        let major = initial >> 5;
        let additional = initial & 0x1f;

        match major {
            0 => Ok(CborValue::Integer(i128::from(
                self.read_argument(additional)?,
            ))),
            1 => {
                let value = self.read_argument(additional)?;
                Ok(CborValue::Integer(-1 - i128::from(value)))
            }
            2 => self.parse_bytes(additional),
            3 => self.parse_text(additional),
            4 => self.parse_array(additional),
            5 => self.parse_map(additional),
            6 => Err(QztError::NonCanonicalCbor),
            7 => self.parse_simple(additional),
            _ => unreachable!("CBOR major type is three bits"),
        }
    }

    fn parse_bytes(&mut self, additional: u8) -> Result<CborValue> {
        let len = self.read_len(additional, MAX_PHASE1_ALLOCATION)?;
        let bytes = self.read_exact(len)?.to_vec();
        Ok(CborValue::Bytes(bytes))
    }

    fn parse_text(&mut self, additional: u8) -> Result<CborValue> {
        let len = self.read_len(additional, MAX_PHASE1_ALLOCATION)?;
        let bytes = self.read_exact(len)?;
        let text = std::str::from_utf8(bytes).map_err(|_| QztError::InvalidUtf8)?;
        Ok(CborValue::Text(text.to_owned()))
    }

    fn parse_array(&mut self, additional: u8) -> Result<CborValue> {
        let len = self.read_len(additional, MAX_PHASE1_ITEMS)?;
        let mut values = Vec::with_capacity(len);
        for _ in 0..len {
            values.push(self.parse_value()?);
        }
        Ok(CborValue::Array(values))
    }

    fn parse_map(&mut self, additional: u8) -> Result<CborValue> {
        let len = self.read_len(additional, MAX_PHASE1_ITEMS)?;
        let mut entries = Vec::with_capacity(len);
        let mut previous_key_bytes: Option<Vec<u8>> = None;

        for _ in 0..len {
            let key_start = self.offset;
            let key = self.parse_value()?;
            let key_bytes = self.input[key_start..self.offset].to_vec();

            if let Some(previous) = &previous_key_bytes {
                match previous.as_slice().cmp(key_bytes.as_slice()) {
                    std::cmp::Ordering::Equal => return Err(QztError::DuplicateCborKey),
                    std::cmp::Ordering::Greater => return Err(QztError::NonCanonicalCbor),
                    std::cmp::Ordering::Less => {}
                }
            }

            previous_key_bytes = Some(key_bytes);
            let value = self.parse_value()?;
            entries.push((key, value));
        }

        Ok(CborValue::Map(entries))
    }

    fn parse_simple(&mut self, additional: u8) -> Result<CborValue> {
        match additional {
            20 => Ok(CborValue::Bool(false)),
            21 => Ok(CborValue::Bool(true)),
            22 => Ok(CborValue::Null),
            _ => Err(QztError::NonCanonicalCbor),
        }
    }

    fn read_len(&mut self, additional: u8, max: u64) -> Result<usize> {
        let len = self.read_argument(additional)?;
        if len > max || len > usize::MAX as u64 {
            return Err(QztError::ResourceLimitExceeded);
        }
        usize::try_from(len).map_err(|_| QztError::ResourceLimitExceeded)
    }

    fn read_argument(&mut self, additional: u8) -> Result<u64> {
        match additional {
            value @ 0..=23 => Ok(u64::from(value)),
            24 => {
                let value = u64::from(self.read_u8()?);
                if value < 24 {
                    return Err(QztError::NonCanonicalCbor);
                }
                Ok(value)
            }
            25 => {
                let value = u64::from(u16::from_be_bytes(self.read_array()?));
                if value <= u64::from(u8::MAX) {
                    return Err(QztError::NonCanonicalCbor);
                }
                Ok(value)
            }
            26 => {
                let value = u64::from(u32::from_be_bytes(self.read_array()?));
                if value <= u64::from(u16::MAX) {
                    return Err(QztError::NonCanonicalCbor);
                }
                Ok(value)
            }
            27 => {
                let value = u64::from_be_bytes(self.read_array()?);
                if value <= u64::from(u32::MAX) {
                    return Err(QztError::NonCanonicalCbor);
                }
                Ok(value)
            }
            28..=31 => Err(QztError::NonCanonicalCbor),
            _ => unreachable!("CBOR additional information is five bits"),
        }
    }

    fn read_u8(&mut self) -> Result<u8> {
        let byte = self
            .input
            .get(self.offset)
            .copied()
            .ok_or(QztError::UnexpectedEof)?;
        self.offset += 1;
        Ok(byte)
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let bytes = self.read_exact(N)?;
        bytes.try_into().map_err(|_| QztError::UnexpectedEof)
    }

    fn read_exact(&mut self, len: usize) -> Result<&[u8]> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(QztError::ResourceLimitExceeded)?;
        let bytes = self
            .input
            .get(self.offset..end)
            .ok_or(QztError::UnexpectedEof)?;
        self.offset = end;
        Ok(bytes)
    }
}
