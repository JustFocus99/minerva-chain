use primitives::error::PrimitiveError;
use state::account::Account;
#[test]
fn account_starts_with_expected_balance() {
    let account = Account::new([7u8; 32], 42);
    assert_eq!(account.get_balance(), 42);
}

#[test]
fn account_starts_with_nonce_zero() {
    let account = Account::new([7u8; 32], 42);
    assert_eq!(account.get_nonce(), 0);
}

#[test]
fn credit_increases_balance() {
    let mut account = Account::new([7u8; 32], 10);
    account.deposit(5).unwrap();
    assert_eq!(account.get_balance(), 15);
}

#[test]
fn debit_decreases_balance() {
    let mut account = Account::new([7u8; 32], 10);
    account.withdraw(3).unwrap();
    assert_eq!(account.get_balance(), 7);
}

#[test]
fn debit_rejects_insufficient_balance() {
    let mut account = Account::new([7u8; 32], 10);
    let err = account.withdraw(11).unwrap_err();
    assert_eq!(err, PrimitiveError::AmountUnderflow);
}

#[test]
fn failed_debit_does_not_mutate_balance() {
    let mut account = Account::new([7u8; 32], 10);
    let _ = account.withdraw(11);
    assert_eq!(account.get_balance(), 10);
}

#[test]
fn credit_rejects_overflow() {
    let mut account = Account::new([7u8; 32], u64::MAX);
    let err = account.deposit(1).unwrap_err();
    assert_eq!(err, PrimitiveError::AmountOverflow);
}

#[test]
fn failed_credit_does_not_mutate_balance() {
    let mut account = Account::new([7u8; 32], u64::MAX);
    let _ = account.deposit(1);
    assert_eq!(account.get_balance(), u64::MAX);
}

#[test]
fn nonce_increments_by_one() {
    let mut account = Account::new([7u8; 32], 10);
    account.increment_nonce();
    assert_eq!(account.get_nonce(), 1);
}

#[test]
fn account_id_equality_works() {
    let account_a = Account::new([7u8; 32], 10);
    let account_b = Account::new([7u8; 32], 20);
    assert_eq!(account_a.get_id(), account_b.get_id());
}
