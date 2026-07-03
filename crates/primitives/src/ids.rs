use crate::error::PrimitiveError;

/// A 32-byte identifier for an account.
pub type AccountId = [u8; 32];

/// A 32-byte identifier for a validator.
pub type ValidatorId = [u8; 32];

/// A 32-byte identifier for a transaction.
pub type TransactionId = [u8; 32];

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_32_byte_account_id_creation_works() {
        let bytes = [1u8; 32];
        let account_id = account_id_from_bytes(&bytes).unwrap();
        assert_eq!(account_id, bytes);
    }

    #[test]
    fn invalid_account_id_length_is_rejected() {
        let bytes = [1u8; 31];
        let err = account_id_from_bytes(&bytes).unwrap_err();
        assert_eq!(
            err,
            PrimitiveError::InvalidLength {
                expected: 32,
                actual: 31,
            }
        );
    }
}
