use primitives::error::PrimitiveError;
use primitives::ids::account_id_from_bytes;

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
