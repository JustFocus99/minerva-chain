use state::account::Account;
use state::chain_state::ChainState;
use state::error::StateError;
use transaction::transaction::{SignedTransaction, UnsignedTransaction};

fn account(id: [u8; 32], balance: u64) -> Account {
    Account::new(id, balance)
}

fn signed_tx(from: [u8; 32], to: [u8; 32], amount: u64, nonce: u64) -> SignedTransaction {
    SignedTransaction::sign(UnsignedTransaction {
        from,
        to,
        amount,
        nonce,
    })
}

#[test]
fn valid_transfer_debits_sender() {
    let mut state = ChainState::new();
    state.create_account(account([1u8; 32], 100));
    state.create_account(account([2u8; 32], 50));

    let signed = signed_tx([1u8; 32], [2u8; 32], 25, 0);

    state.apply_signed_transaction(signed).unwrap();

    assert_eq!(state.get_account(&[1u8; 32]).unwrap().balance, 75);
}

#[test]
fn valid_transfer_credits_receiver() {
    let mut state = ChainState::new();
    state.create_account(account([1u8; 32], 100));
    state.create_account(account([2u8; 32], 50));

    let signed = signed_tx([1u8; 32], [2u8; 32], 25, 0);

    state.apply_signed_transaction(signed).unwrap();

    assert_eq!(state.get_account(&[2u8; 32]).unwrap().balance, 75);
}

#[test]
fn valid_transfer_increments_sender_nonce() {
    let mut state = ChainState::new();
    state.create_account(account([1u8; 32], 100));
    state.create_account(account([2u8; 32], 50));

    let signed = signed_tx([1u8; 32], [2u8; 32], 25, 0);

    state.apply_signed_transaction(signed).unwrap();

    assert_eq!(state.get_account(&[1u8; 32]).unwrap().nonce, 1);
}

#[test]
fn total_supply_unchanged_after_valid_transfer() {
    let mut state = ChainState::new();
    state.create_account(account([1u8; 32], 100));
    state.create_account(account([2u8; 32], 50));

    let signed = signed_tx([1u8; 32], [2u8; 32], 25, 0);

    state.apply_signed_transaction(signed).unwrap();

    assert_eq!(state.total_supply(), 150);
}

#[test]
fn missing_sender_fails() {
    let mut state = ChainState::new();
    state.create_account(account([2u8; 32], 50));

    let signed = signed_tx([1u8; 32], [2u8; 32], 25, 0);

    let err = state.apply_signed_transaction(signed).unwrap_err();
    assert!(matches!(err, StateError::SenderMissing));
}

#[test]
fn missing_receiver_fails() {
    let mut state = ChainState::new();
    state.create_account(account([1u8; 32], 100));

    let signed = signed_tx([1u8; 32], [2u8; 32], 25, 0);

    let err = state.apply_signed_transaction(signed).unwrap_err();
    assert!(matches!(err, StateError::ReceiverMissing));
}

#[test]
fn invalid_signature_fails() {
    let mut state = ChainState::new();
    state.create_account(account([1u8; 32], 100));
    state.create_account(account([2u8; 32], 50));

    let mut signed = signed_tx([1u8; 32], [2u8; 32], 25, 0);
    signed.public_key = [9u8; 32];

    let err = state.apply_signed_transaction(signed).unwrap_err();
    assert!(matches!(err, StateError::InvalidSignature));
}

#[test]
fn invalid_signature_does_not_mutate_state() {
    let mut state = ChainState::new();
    state.create_account(account([1u8; 32], 100));
    state.create_account(account([2u8; 32], 50));

    let mut signed = signed_tx([1u8; 32], [2u8; 32], 25, 0);
    signed.public_key = [9u8; 32];

    state.apply_signed_transaction(signed).unwrap_err();

    assert_eq!(state.get_account(&[1u8; 32]).unwrap().balance, 100);
    assert_eq!(state.get_account(&[2u8; 32]).unwrap().balance, 50);
    assert_eq!(state.get_account(&[1u8; 32]).unwrap().nonce, 0);
}

#[test]
fn wrong_nonce_fails() {
    let mut state = ChainState::new();
    state.create_account(account([1u8; 32], 100));
    state.create_account(account([2u8; 32], 50));

    let signed = signed_tx([1u8; 32], [2u8; 32], 25, 7);

    let err = state.apply_signed_transaction(signed).unwrap_err();
    assert!(matches!(err, StateError::InvalidNonce { .. }));
}

#[test]
fn wrong_nonce_does_not_mutate_state() {
    let mut state = ChainState::new();
    state.create_account(account([1u8; 32], 100));
    state.create_account(account([2u8; 32], 50));

    let signed = signed_tx([1u8; 32], [2u8; 32], 25, 7);

    state.apply_signed_transaction(signed).unwrap_err();

    assert_eq!(state.get_account(&[1u8; 32]).unwrap().balance, 100);
    assert_eq!(state.get_account(&[2u8; 32]).unwrap().balance, 50);
    assert_eq!(state.get_account(&[1u8; 32]).unwrap().nonce, 0);
}

#[test]
fn insufficient_balance_fails() {
    let mut state = ChainState::new();
    state.create_account(account([1u8; 32], 100));
    state.create_account(account([2u8; 32], 50));

    let signed = signed_tx([1u8; 32], [2u8; 32], 200, 0);

    let err = state.apply_signed_transaction(signed).unwrap_err();
    assert!(matches!(err, StateError::InsufficientBalance { .. }));
}

#[test]
fn insufficient_balance_does_not_mutate_state() {
    let mut state = ChainState::new();
    state.create_account(account([1u8; 32], 100));
    state.create_account(account([2u8; 32], 50));

    let signed = signed_tx([1u8; 32], [2u8; 32], 200, 0);

    state.apply_signed_transaction(signed).unwrap_err();

    assert_eq!(state.get_account(&[1u8; 32]).unwrap().balance, 100);
    assert_eq!(state.get_account(&[2u8; 32]).unwrap().balance, 50);
    assert_eq!(state.get_account(&[1u8; 32]).unwrap().nonce, 0);
}

#[test]
fn zero_amount_fails() {
    let mut state = ChainState::new();
    state.create_account(account([1u8; 32], 100));
    state.create_account(account([2u8; 32], 50));

    let signed = signed_tx([1u8; 32], [2u8; 32], 0, 0);

    let err = state.apply_signed_transaction(signed).unwrap_err();
    assert!(matches!(err, StateError::ZeroAmount));
}

#[test]
fn sender_equals_receiver_fails() {
    let mut state = ChainState::new();
    state.create_account(account([1u8; 32], 100));

    let signed = signed_tx([1u8; 32], [1u8; 32], 25, 0);

    let err = state.apply_signed_transaction(signed).unwrap_err();
    assert!(matches!(err, StateError::SenderEqualsReceiver));
}

#[test]
fn replaying_same_transaction_fails_because_nonce_changed() {
    let mut state = ChainState::new();
    state.create_account(account([1u8; 32], 100));
    state.create_account(account([2u8; 32], 50));

    let first = signed_tx([1u8; 32], [2u8; 32], 25, 0);
    let second = signed_tx([1u8; 32], [2u8; 32], 25, 0);

    state.apply_signed_transaction(first).unwrap();
    let err = state.apply_signed_transaction(second).unwrap_err();

    assert!(matches!(err, StateError::InvalidNonce { .. }));
}
