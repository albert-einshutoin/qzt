use qzt::cbor::{
    encode_deterministic, validate_deterministic, validate_text_key_schema, CborValue,
    TextKeySchema,
};
use qzt::error::QztError;

#[test]
fn accepts_canonical_text_keyed_map() {
    let value = validate_deterministic(&[0xa2, 0x61, b'a', 0x01, 0x61, b'b', 0x82, 0xf4, 0xf6])
        .expect("canonical map should pass");

    assert_eq!(
        value,
        CborValue::Map(vec![
            (CborValue::Text("a".to_owned()), CborValue::Integer(1)),
            (
                CborValue::Text("b".to_owned()),
                CborValue::Array(vec![CborValue::Bool(false), CborValue::Null])
            )
        ])
    );
}

#[test]
fn rejects_non_shortest_integer_encoding() {
    assert_eq!(
        validate_deterministic(&[0x18, 0x17]),
        Err(QztError::NonCanonicalCbor)
    );
}

#[test]
fn rejects_duplicate_map_keys() {
    assert_eq!(
        validate_deterministic(&[0xa2, 0x61, b'a', 0x01, 0x61, b'a', 0x02]),
        Err(QztError::DuplicateCborKey)
    );
}

#[test]
fn rejects_unsorted_map_keys() {
    assert_eq!(
        validate_deterministic(&[0xa2, 0x61, b'b', 0x01, 0x61, b'a', 0x02]),
        Err(QztError::NonCanonicalCbor)
    );
}

#[test]
fn rejects_indefinite_length_strings() {
    assert_eq!(
        validate_deterministic(&[0x7f, 0x61, b'a', 0xff]),
        Err(QztError::NonCanonicalCbor)
    );
}

#[test]
fn rejects_tags_and_floats() {
    assert_eq!(
        validate_deterministic(&[0xc0, 0x00]),
        Err(QztError::NonCanonicalCbor)
    );
    assert_eq!(
        validate_deterministic(&[0xf9, 0x00, 0x00]),
        Err(QztError::NonCanonicalCbor)
    );
}

#[test]
fn rejects_unknown_closed_schema_field() {
    let schema = TextKeySchema {
        required: &["known"],
        optional: &[],
        allow_unknown: false,
    };

    assert_eq!(
        validate_text_key_schema(
            &[
                0xa2, 0x65, b'k', b'n', b'o', b'w', b'n', 0x01, 0x67, b'u', b'n', b'k', b'n', b'o',
                b'w', b'n', 0x02
            ],
            schema
        ),
        Err(QztError::MetadataInvalid)
    );
}

#[test]
fn rejects_missing_required_closed_schema_field() {
    let schema = TextKeySchema {
        required: &["required"],
        optional: &["optional"],
        allow_unknown: false,
    };

    assert_eq!(
        validate_text_key_schema(
            &[0xa1, 0x68, b'o', b'p', b't', b'i', b'o', b'n', b'a', b'l', 0x01],
            schema
        ),
        Err(QztError::MetadataInvalid)
    );
}

#[test]
fn encoder_uses_shortest_integer_encodings() {
    assert_eq!(
        encode_deterministic(&CborValue::Integer(23)),
        Ok(vec![0x17])
    );
    assert_eq!(
        encode_deterministic(&CborValue::Integer(24)),
        Ok(vec![0x18, 0x18])
    );
    assert_eq!(
        encode_deterministic(&CborValue::Integer(256)),
        Ok(vec![0x19, 0x01, 0x00])
    );
}

#[test]
fn encoder_handles_negative_integer_boundaries_without_panics() {
    assert_eq!(
        encode_deterministic(&CborValue::Integer(-1)),
        Ok(vec![0x20])
    );
    assert_eq!(
        encode_deterministic(&CborValue::Integer(i128::MIN)),
        Err(QztError::ResourceLimitExceeded)
    );
}

#[test]
fn encoder_sorts_map_keys_by_encoded_bytes() {
    let value = CborValue::Map(vec![
        (CborValue::Text("b".to_owned()), CborValue::Integer(2)),
        (CborValue::Text("a".to_owned()), CborValue::Integer(1)),
    ]);

    assert_eq!(
        encode_deterministic(&value),
        Ok(vec![0xa2, 0x61, b'a', 0x01, 0x61, b'b', 0x02])
    );
}

#[test]
fn encoder_rejects_duplicate_map_keys() {
    let value = CborValue::Map(vec![
        (CborValue::Text("a".to_owned()), CborValue::Integer(1)),
        (CborValue::Text("a".to_owned()), CborValue::Integer(2)),
    ]);

    assert_eq!(
        encode_deterministic(&value),
        Err(QztError::DuplicateCborKey)
    );
}
