//! On-disk record framing. See `docs/storage.md` for the format this module
//! implements: `[magic][version][length][height][hash][payload][crc32][commit marker]`.

use block::block::{Block, BlockHeader};
use primitives::{AccountId, Amount, BlockHash, Nonce, PublicKeyBytes, SignatureBytes};
use transaction::transaction::{SignedTransaction, UnsignedTransaction};

use crate::error::StorageError;

pub const MAGIC: [u8; 4] = *b"MINC";
pub const VERSION: u8 = 1;
pub const COMMIT_MARKER: u8 = 0x01;

/// Provisional upper bound on a single block's encoded payload size.
/// See "Open questions" in `docs/storage.md` — no real block-size budget
/// has been defined yet.
pub const MAX_PAYLOAD_LEN: u32 = 16 * 1024 * 1024;

/// magic(4) + version(1) + length(4) + height(8) + hash(32)
const FIXED_PREFIX_LEN: usize = 4 + 1 + 4 + 8 + 32;
/// crc32(4) + commit marker(1)
const FIXED_SUFFIX_LEN: usize = 4 + 1;

/// Reads `N` bytes at `*cursor`, advancing it, without ever panicking on
/// short input.
fn read_array<const N: usize>(bytes: &[u8], cursor: &mut usize) -> Result<[u8; N], String> {
    let end = *cursor + N;
    if end > bytes.len() {
        return Err(format!(
            "expected {N} bytes at offset {cursor}, found {}",
            bytes.len() - *cursor
        ));
    }
    let mut array = [0u8; N];
    array.copy_from_slice(&bytes[*cursor..end]);
    *cursor = end;
    Ok(array)
}

/// Fixed-size wire encoding for one `SignedTransaction`: `from(32) +
/// to(32) + amount(8 LE) + nonce(8 LE) + public_key(32) + signature(64)`.
/// Used both inside a block's payload (`encode_block`/`decode_block`) and
/// fuzzed on its own -- see `fuzz/fuzz_targets/transaction_decoder.rs`.
pub const TRANSACTION_RECORD_LEN: usize = 32 + 32 + 8 + 8 + 32 + 64;

pub fn encode_signed_transaction(tx: &SignedTransaction) -> [u8; TRANSACTION_RECORD_LEN] {
    let mut buf = [0u8; TRANSACTION_RECORD_LEN];
    buf[0..32].copy_from_slice(&tx.transaction.from);
    buf[32..64].copy_from_slice(&tx.transaction.to);
    buf[64..72].copy_from_slice(&tx.transaction.amount.to_le_bytes());
    buf[72..80].copy_from_slice(&tx.transaction.nonce.to_le_bytes());
    buf[80..112].copy_from_slice(&tx.public_key);
    buf[112..176].copy_from_slice(&tx.signature);
    buf
}

/// Inverse of [`encode_signed_transaction`]. Rejects anything that isn't
/// exactly `TRANSACTION_RECORD_LEN` bytes; every other step is a fixed-size
/// array copy or a `from_le_bytes` parse, neither of which can fail or
/// panic once the length check passes -- `bytes` is otherwise
/// unconstrained. This is a *structural* decode only: it does not check
/// that the signature verifies, that `from` names a real account, or
/// anything else `SignedTransaction::verify` /
/// `state::ChainState::apply_signed_transaction` are responsible for.
pub fn decode_signed_transaction(bytes: &[u8]) -> Result<SignedTransaction, StorageError> {
    let bytes: &[u8; TRANSACTION_RECORD_LEN] =
        bytes
            .try_into()
            .map_err(|_| StorageError::Decode {
                offset: 0,
                reason: format!(
                    "transaction record must be exactly {TRANSACTION_RECORD_LEN} bytes, found {}",
                    bytes.len()
                ),
            })?;

    let mut from: AccountId = [0u8; 32];
    from.copy_from_slice(&bytes[0..32]);
    let mut to: AccountId = [0u8; 32];
    to.copy_from_slice(&bytes[32..64]);
    let amount = Amount::from_le_bytes(bytes[64..72].try_into().unwrap());
    let nonce = Nonce::from_le_bytes(bytes[72..80].try_into().unwrap());
    let mut public_key: PublicKeyBytes = [0u8; 32];
    public_key.copy_from_slice(&bytes[80..112]);
    let mut signature: SignatureBytes = [0u8; 64];
    signature.copy_from_slice(&bytes[112..176]);

    Ok(SignedTransaction {
        transaction: UnsignedTransaction { from, to, amount, nonce },
        public_key,
        signature,
    })
}

/// Deterministically encodes a block's header and transactions into the
/// record's `payload` field. `block_hash` is not stored here — it is
/// already carried in the record's fixed `hash` field, and is re-derived
/// from the other header fields by `BlockHeader::new` on decode.
fn encode_block(block: &Block) -> Vec<u8> {
    let header = &block.header;
    let mut buf = Vec::new();

    buf.extend_from_slice(&header.height.to_le_bytes());
    buf.extend_from_slice(&header.parent_hash);
    buf.extend_from_slice(header.transaction_root.as_bytes());
    buf.extend_from_slice(&header.state_commitment);
    buf.extend_from_slice(&header.producer);
    buf.extend_from_slice(&header.slot.to_le_bytes());

    buf.extend_from_slice(&(block.transactions.len() as u32).to_le_bytes());
    for tx in &block.transactions {
        buf.extend_from_slice(&encode_signed_transaction(tx));
    }

    buf
}

/// Inverse of [`encode_block`]. Returns a plain `String` reason on failure;
/// the caller (`decode_record`) is the one with offset context to turn it
/// into a [`StorageError`].
fn decode_block(payload: &[u8]) -> Result<Block, String> {
    let mut cursor = 0usize;

    let height = u64::from_le_bytes(read_array::<8>(payload, &mut cursor)?);
    let parent_hash: BlockHash = read_array::<32>(payload, &mut cursor)?;
    let transaction_root_bytes = read_array::<32>(payload, &mut cursor)?;
    let state_commitment = read_array::<32>(payload, &mut cursor)?;
    let producer = read_array::<32>(payload, &mut cursor)?;
    let slot = u64::from_le_bytes(read_array::<8>(payload, &mut cursor)?);

    let header = BlockHeader::new(
        height,
        parent_hash,
        primitives::TransactionRoot::new(transaction_root_bytes),
        state_commitment,
        producer,
        slot,
    );

    let tx_count = u32::from_le_bytes(read_array::<4>(payload, &mut cursor)?);
    // `tx_count` is untrusted, attacker-controlled input -- pre-allocating
    // `Vec::with_capacity(tx_count as usize)` before a single byte of the
    // claimed transactions has actually been read would let 4 bytes of
    // input request an arbitrarily large allocation (a denial-of-service,
    // not a memory-safety bug, but a real crash: `Vec::with_capacity`
    // aborts the process on allocation failure rather than returning an
    // `Err`). Growing from empty means the only way to add an entry is to
    // first successfully decode `TRANSACTION_RECORD_LEN` real bytes for
    // it via `read_array`'s bounds check below.
    let mut transactions = Vec::new();
    for _ in 0..tx_count {
        let record = read_array::<TRANSACTION_RECORD_LEN>(payload, &mut cursor)?;
        transactions.push(decode_signed_transaction(&record).map_err(|err| err.to_string())?);
    }

    if cursor != payload.len() {
        return Err(format!(
            "{} trailing byte(s) after decoding {tx_count} transaction(s)",
            payload.len() - cursor
        ));
    }

    Ok(Block {
        header,
        transactions,
    })
}

/// Encodes everything in a record except the trailing commit marker byte:
/// `[magic][version][length][height][hash][payload][crc32]`.
pub fn encode_record_without_marker(block: &Block) -> Result<Vec<u8>, StorageError> {
    let payload = encode_block(block);
    let length: u32 = payload
        .len()
        .try_into()
        .map_err(|_| StorageError::PayloadTooLarge {
            length: payload.len(),
            max: MAX_PAYLOAD_LEN,
        })?;
    if length == 0 || length > MAX_PAYLOAD_LEN {
        return Err(StorageError::PayloadTooLarge {
            length: payload.len(),
            max: MAX_PAYLOAD_LEN,
        });
    }

    let mut body = Vec::with_capacity(FIXED_PREFIX_LEN + payload.len() + 4);
    body.extend_from_slice(&MAGIC);
    body.push(VERSION);
    body.extend_from_slice(&length.to_le_bytes());
    body.extend_from_slice(&block.header.height.to_le_bytes());
    body.extend_from_slice(&block.header.block_hash);
    body.extend_from_slice(&payload);

    let mut crc_input = Vec::with_capacity(8 + 32 + payload.len());
    crc_input.extend_from_slice(&block.header.height.to_le_bytes());
    crc_input.extend_from_slice(&block.header.block_hash);
    crc_input.extend_from_slice(&payload);
    body.extend_from_slice(&crc32(&crc_input).to_le_bytes());

    Ok(body)
}

/// Decodes a single record starting at `offset` in `bytes`. Validity is
/// checked in the order documented in `docs/storage.md`: magic, version,
/// length sanity, payload decode, checksum, block hash match, commit
/// marker. Returns the decoded block and the total number of bytes the
/// record occupied (so the caller can advance to the next record).
pub fn decode_record(bytes: &[u8], offset: usize) -> Result<(Block, usize), StorageError> {
    let remaining = bytes.len() - offset;

    if remaining < 4 {
        return Err(StorageError::Truncated {
            offset,
            expected: 4,
            found: remaining,
        });
    }
    if bytes[offset..offset + 4] != MAGIC {
        return Err(StorageError::MagicMismatch { offset });
    }

    if remaining < 5 {
        return Err(StorageError::Truncated {
            offset,
            expected: 5,
            found: remaining,
        });
    }
    let version = bytes[offset + 4];
    if version != VERSION {
        return Err(StorageError::UnsupportedVersion { offset, version });
    }

    if remaining < FIXED_PREFIX_LEN {
        return Err(StorageError::Truncated {
            offset,
            expected: FIXED_PREFIX_LEN,
            found: remaining,
        });
    }
    let mut length_bytes = [0u8; 4];
    length_bytes.copy_from_slice(&bytes[offset + 5..offset + 9]);
    let length = u32::from_le_bytes(length_bytes);
    if length == 0 || length > MAX_PAYLOAD_LEN {
        return Err(StorageError::InvalidLength {
            offset,
            length,
            max: MAX_PAYLOAD_LEN,
        });
    }

    let record_len = FIXED_PREFIX_LEN + length as usize + FIXED_SUFFIX_LEN;
    if remaining < record_len {
        return Err(StorageError::Truncated {
            offset,
            expected: record_len,
            found: remaining,
        });
    }

    let mut height_bytes = [0u8; 8];
    height_bytes.copy_from_slice(&bytes[offset + 9..offset + 17]);
    let height = u64::from_le_bytes(height_bytes);

    let mut hash = [0u8; 32];
    hash.copy_from_slice(&bytes[offset + 17..offset + 49]);

    let payload_start = offset + FIXED_PREFIX_LEN;
    let payload_end = payload_start + length as usize;
    let payload = &bytes[payload_start..payload_end];

    let mut crc_bytes = [0u8; 4];
    crc_bytes.copy_from_slice(&bytes[payload_end..payload_end + 4]);
    let stored_crc = u32::from_le_bytes(crc_bytes);

    let commit_marker = bytes[payload_end + 4];

    let block = decode_block(payload).map_err(|reason| StorageError::Decode { offset, reason })?;

    let mut crc_input = Vec::with_capacity(8 + 32 + payload.len());
    crc_input.extend_from_slice(&height_bytes);
    crc_input.extend_from_slice(&hash);
    crc_input.extend_from_slice(payload);
    if crc32(&crc_input) != stored_crc {
        return Err(StorageError::ChecksumMismatch { offset });
    }

    if block.header.height != height || block.header.block_hash != hash {
        return Err(StorageError::BlockHashMismatch {
            offset,
            declared: hash,
            computed: block.header.block_hash,
        });
    }

    if commit_marker != COMMIT_MARKER {
        return Err(StorageError::MissingCommitMarker { offset });
    }

    Ok((block, record_len))
}

/// Bit-by-bit CRC-32/ISO-HDLC (the common "CRC32" used by zip/gzip/ethernet).
/// Table-free by design — this log is small enough that the simplicity is
/// worth more than the speed of a lookup table.
fn crc32(data: &[u8]) -> u32 {
    const POLY: u32 = 0xEDB8_8320;
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (POLY & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_known_check_value() {
        // Standard CRC-32/ISO-HDLC check value for the ASCII string "123456789".
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }
}
