use crate::error::PrimitiveError;

/// A 32-byte identifier for an account.
pub type AccountId = [u8; 32];

/// A 32-byte identifier for a validator.
pub type ValidatorId = [u8; 32];

/// Create an account ID from a byte slice.
pub fn account_id_from_bytes(bytes: &[u8]) -> Result<AccountId, PrimitiveError> {
    if bytes.len() != 32 {
        return Err(PrimitiveError::InvalidLength {
            expected: 32,
            actual: bytes.len(),
        });
    }

    let mut account_id = [0u8; 32];
    account_id.copy_from_slice(bytes);
    Ok(account_id)
}
