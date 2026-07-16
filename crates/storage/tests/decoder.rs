//! Hour 4 — malformed-bytes protection. The decoder is the surface a fuzzer
//! will attack first, so every rejection path gets an explicit test: no
//! byte sequence should be silently accepted as a valid record.

mod common;

use block::GENESIS_PARENT_HASH;
use storage::error::StorageError;
use storage::record;

#[test]
fn rejects_empty_file() {
    let bytes: Vec<u8> = Vec::new();

    let err = record::decode_record(&bytes, 0).expect_err("empty input must be rejected");
    assert!(matches!(err, StorageError::Truncated { .. }));
}

#[test]
fn rejects_random_bytes() {
    let bytes = vec![0xABu8; 64];

    let err = record::decode_record(&bytes, 0).expect_err("random bytes must be rejected");
    assert!(matches!(err, StorageError::MagicMismatch { .. }));
}

#[test]
fn rejects_wrong_magic() {
    let block = common::sample_block(0, GENESIS_PARENT_HASH, 1);
    let mut bytes = record::encode_record_without_marker(&block).expect("encode block");
    bytes.push(record::COMMIT_MARKER);
    bytes[0..4].copy_from_slice(b"XXXX");

    let err = record::decode_record(&bytes, 0).expect_err("wrong magic must be rejected");
    assert!(matches!(err, StorageError::MagicMismatch { .. }));
}

#[test]
fn rejects_wrong_version() {
    let block = common::sample_block(0, GENESIS_PARENT_HASH, 1);
    let mut bytes = record::encode_record_without_marker(&block).expect("encode block");
    bytes.push(record::COMMIT_MARKER);
    bytes[4] = 99;

    let err = record::decode_record(&bytes, 0).expect_err("unsupported version must be rejected");
    assert!(matches!(
        err,
        StorageError::UnsupportedVersion { version: 99, .. }
    ));
}

#[test]
fn rejects_wrong_length() {
    let block = common::sample_block(0, GENESIS_PARENT_HASH, 1);
    let mut bytes = record::encode_record_without_marker(&block).expect("encode block");
    bytes.push(record::COMMIT_MARKER);
    // A length this large blows past MAX_PAYLOAD_LEN before any attempt to
    // read that many bytes is made.
    bytes[5..9].copy_from_slice(&u32::MAX.to_le_bytes());

    let err = record::decode_record(&bytes, 0).expect_err("insane length must be rejected");
    assert!(matches!(err, StorageError::InvalidLength { .. }));
}

#[test]
fn rejects_bad_checksum() {
    let block = common::sample_block(0, GENESIS_PARENT_HASH, 1);
    let mut bytes = record::encode_record_without_marker(&block).expect("encode block");
    bytes.push(record::COMMIT_MARKER);

    // Flip a byte inside the payload, well clear of magic/version/length/
    // height/hash, so only the checksum is affected.
    let payload_start = 49; // FIXED_PREFIX_LEN: magic(4)+version(1)+length(4)+height(8)+hash(32)
    bytes[payload_start] ^= 0xFF;

    let err = record::decode_record(&bytes, 0).expect_err("corrupted payload must be rejected");
    assert!(matches!(err, StorageError::ChecksumMismatch { .. }));
}

#[test]
fn rejects_truncated_payload() {
    let block = common::sample_block(0, GENESIS_PARENT_HASH, 1);
    let mut bytes = record::encode_record_without_marker(&block).expect("encode block");
    bytes.push(record::COMMIT_MARKER);

    // Chop off the tail mid-payload, well before crc32 or the commit marker.
    bytes.truncate(bytes.len() - 10);

    let err = record::decode_record(&bytes, 0).expect_err("truncated payload must be rejected");
    assert!(matches!(err, StorageError::Truncated { .. }));
}
