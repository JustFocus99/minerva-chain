use state::account::Account;
use state::chain_state::ChainState;
use state::error::StateError;
use transaction::transaction::{SignedTransaction, UnsignedTransaction};

const FEE_COLLECTOR: [u8; 32] = [7u8; 32];

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
    state.create_account(account(FEE_COLLECTOR, 0));
    state.set_fee_collector(FEE_COLLECTOR);

    let signed = signed_tx([1u8; 32], [2u8; 32], 25, 0);

    state.apply_signed_transaction(signed).unwrap();

    assert_eq!(state.get_account(&[1u8; 32]).unwrap().balance, 74);
}

#[test]
fn valid_transfer_credits_receiver() {
    let mut state = ChainState::new();
    state.create_account(account([1u8; 32], 100));
    state.create_account(account([2u8; 32], 50));
    state.create_account(account(FEE_COLLECTOR, 0));
    state.set_fee_collector(FEE_COLLECTOR);

    let signed = signed_tx([1u8; 32], [2u8; 32], 25, 0);

    state.apply_signed_transaction(signed).unwrap();

    assert_eq!(state.get_account(&[2u8; 32]).unwrap().balance, 75);
}

#[test]
fn valid_transfer_increments_sender_nonce() {
    let mut state = ChainState::new();
    state.create_account(account([1u8; 32], 100));
    state.create_account(account([2u8; 32], 50));
    state.create_account(account(FEE_COLLECTOR, 0));
    state.set_fee_collector(FEE_COLLECTOR);

    let signed = signed_tx([1u8; 32], [2u8; 32], 25, 0);

    state.apply_signed_transaction(signed).unwrap();

    assert_eq!(state.get_account(&[1u8; 32]).unwrap().nonce, 1);
}

#[test]
fn total_supply_unchanged_after_valid_transfer() {
    let mut state = ChainState::new();
    state.create_account(account([1u8; 32], 100));
    state.create_account(account([2u8; 32], 50));
    state.create_account(account(FEE_COLLECTOR, 0));
    state.set_fee_collector(FEE_COLLECTOR);

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
    state.create_account(account(FEE_COLLECTOR, 0));
    state.set_fee_collector(FEE_COLLECTOR);

    let first = signed_tx([1u8; 32], [2u8; 32], 25, 0);
    let second = signed_tx([1u8; 32], [2u8; 32], 25, 0);

    state.apply_signed_transaction(first).unwrap();
    let err = state.apply_signed_transaction(second).unwrap_err();

    assert!(matches!(err, StateError::InvalidNonce { .. }));
}

#[test]
fn successful_transaction_charges_fee() {
    let mut state = ChainState::new();
    state.create_account(account([1u8; 32], 100));
    state.create_account(account([2u8; 32], 0));
    state.create_account(account(FEE_COLLECTOR, 0));
    state.set_fee_collector(FEE_COLLECTOR);

    let signed = signed_tx([1u8; 32], [2u8; 32], 10, 0);

    state.apply_signed_transaction(signed).unwrap();

    assert_eq!(state.get_account(&[1u8; 32]).unwrap().balance, 89);
    assert_eq!(state.get_account(&[2u8; 32]).unwrap().balance, 10);
    assert_eq!(state.get_account(&FEE_COLLECTOR).unwrap().balance, 1);
}

#[test]
fn insufficient_fee_balance_rejects_transaction() {
    let mut state = ChainState::new();
    state.create_account(account([1u8; 32], 10));
    state.create_account(account([2u8; 32], 0));
    state.create_account(account(FEE_COLLECTOR, 0));
    state.set_fee_collector(FEE_COLLECTOR);

    // Alice has exactly enough for the transfer amount, but not amount + fee.
    let signed = signed_tx([1u8; 32], [2u8; 32], 10, 0);

    let err = state.apply_signed_transaction(signed).unwrap_err();

    assert!(matches!(err, StateError::InsufficientBalance { .. }));
    assert_eq!(state.get_account(&[1u8; 32]).unwrap().balance, 10);
    assert_eq!(state.get_account(&[2u8; 32]).unwrap().balance, 0);
    assert_eq!(state.get_account(&FEE_COLLECTOR).unwrap().balance, 0);
}

#[test]
fn fee_overflow_rejects_transaction() {
    let mut state = ChainState::new();
    state.create_account(account([1u8; 32], u64::MAX));
    state.create_account(account([2u8; 32], 0));
    state.create_account(account(FEE_COLLECTOR, 0));
    state.set_fee_collector(FEE_COLLECTOR);

    let signed = signed_tx([1u8; 32], [2u8; 32], u64::MAX, 0);

    let err = state.apply_signed_transaction(signed).unwrap_err();

    assert!(matches!(err, StateError::Amount(_)));
    assert_eq!(state.get_account(&[1u8; 32]).unwrap().balance, u64::MAX);
    assert_eq!(state.get_account(&[2u8; 32]).unwrap().balance, 0);
    assert_eq!(state.get_account(&FEE_COLLECTOR).unwrap().balance, 0);
}

// --- Day 2, Hour 5: fee execution atomicity ---
//
// apply_signed_transaction already computes every new balance with checked
// arithmetic (total_debit, sender_new_balance, receiver_new_balance,
// fee_collector_new_balance) before any of the three get_account_mut calls
// that actually assign them -- so "no mutation before all calculations
// succeed" already holds structurally. These tests exercise that from the
// outside.

#[test]
fn failed_fee_payment_does_not_mutate_state() {
    let mut state = ChainState::new();
    // Enough for the transfer amount alone, not amount + fee.
    state.create_account(account([1u8; 32], 10));
    state.create_account(account([2u8; 32], 0));
    state.create_account(account(FEE_COLLECTOR, 0));
    state.set_fee_collector(FEE_COLLECTOR);

    let signed = signed_tx([1u8; 32], [2u8; 32], 10, 0);
    let err = state.apply_signed_transaction(signed).unwrap_err();

    assert!(matches!(err, StateError::InsufficientBalance { .. }));
    assert_eq!(state.get_account(&[1u8; 32]).unwrap().balance, 10);
    assert_eq!(state.get_account(&[1u8; 32]).unwrap().nonce, 0);
    assert_eq!(state.get_account(&[2u8; 32]).unwrap().balance, 0);
    assert_eq!(state.get_account(&FEE_COLLECTOR).unwrap().balance, 0);
}

#[test]
fn fee_collector_receives_fee_after_successful_transaction() {
    let mut state = ChainState::new();
    state.create_account(account([1u8; 32], 100));
    state.create_account(account([2u8; 32], 0));
    state.create_account(account(FEE_COLLECTOR, 0));
    state.set_fee_collector(FEE_COLLECTOR);

    let signed = signed_tx([1u8; 32], [2u8; 32], 10, 0);
    state.apply_signed_transaction(signed).unwrap();

    assert_eq!(state.get_account(&FEE_COLLECTOR).unwrap().balance, 1);
}

#[test]
fn transfer_and_fee_are_atomic() {
    let mut state = ChainState::new();
    // Exactly enough to cover the transfer amount by itself, but not
    // amount + fee together -- if the transfer and fee debit were checked
    // separately against the starting balance instead of together, this
    // case would wrongly be allowed through.
    state.create_account(account([1u8; 32], 15));
    state.create_account(account([2u8; 32], 0));
    state.create_account(account(FEE_COLLECTOR, 0));
    state.set_fee_collector(FEE_COLLECTOR);

    let signed = signed_tx([1u8; 32], [2u8; 32], 15, 0);
    let err = state.apply_signed_transaction(signed).unwrap_err();

    assert!(matches!(err, StateError::InsufficientBalance { .. }));
    // Neither side moved: not "transfer went through but fee didn't," or
    // the reverse.
    assert_eq!(state.get_account(&[1u8; 32]).unwrap().balance, 15);
    assert_eq!(state.get_account(&[2u8; 32]).unwrap().balance, 0);
    assert_eq!(state.get_account(&FEE_COLLECTOR).unwrap().balance, 0);
}

#[test]
fn missing_fee_collector_rejects_transaction() {
    let mut state = ChainState::new();
    state.create_account(account([1u8; 32], 100));
    state.create_account(account([2u8; 32], 0));

    let signed = signed_tx([1u8; 32], [2u8; 32], 10, 0);

    let err = state.apply_signed_transaction(signed).unwrap_err();

    assert!(matches!(err, StateError::FeeCollectorMissing));
    assert_eq!(state.get_account(&[1u8; 32]).unwrap().balance, 100);
    assert_eq!(state.get_account(&[2u8; 32]).unwrap().balance, 0);
}
