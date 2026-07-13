use state::account::Account;
use state::chain_state::ChainState;
use state::error::StateError;
use state::snapshot::StateSnapshot;
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

fn canonical_state() -> ChainState {
    let mut state = ChainState::new();
    state.create_account(account([1u8; 32], 100));
    state.create_account(account([2u8; 32], 50));
    state.create_account(account(FEE_COLLECTOR, 0));
    state.set_fee_collector(FEE_COLLECTOR);
    state
}

#[test]
fn snapshot_execution_does_not_mutate_canonical_state_before_commit() {
    let canonical = canonical_state();
    let mut snapshot = StateSnapshot::from_canonical(&canonical);

    snapshot
        .apply_signed_transaction(signed_tx([1u8; 32], [2u8; 32], 10, 0))
        .unwrap();

    // The snapshot's candidate has moved, but canonical is a separate,
    // untouched clone -- it must still reflect pre-execution values.
    assert_eq!(canonical.get_account(&[1u8; 32]).unwrap().balance, 100);
    assert_eq!(canonical.get_account(&[2u8; 32]).unwrap().balance, 50);
    assert_eq!(canonical.get_account(&[1u8; 32]).unwrap().nonce, 0);

    // The candidate, on the other hand, did move.
    assert_eq!(
        snapshot
            .candidate()
            .get_account(&[1u8; 32])
            .unwrap()
            .balance,
        89
    );
}

#[test]
fn failed_snapshot_execution_discards_changes() {
    let canonical = canonical_state();
    let mut snapshot = StateSnapshot::from_canonical(&canonical);

    // First transaction succeeds and mutates the candidate.
    snapshot
        .apply_signed_transaction(signed_tx([1u8; 32], [2u8; 32], 10, 0))
        .unwrap();

    // Second transaction replays the same nonce and fails.
    let err = snapshot
        .apply_signed_transaction(signed_tx([1u8; 32], [2u8; 32], 10, 0))
        .unwrap_err();
    assert!(matches!(err, StateError::InvalidNonce { .. }));

    // The snapshot (including its partial mutation from the first tx) is
    // simply dropped here -- into_state() is never called, so there is
    // nothing to commit and nothing to explicitly roll back.
    drop(snapshot);

    assert_eq!(canonical.get_account(&[1u8; 32]).unwrap().balance, 100);
    assert_eq!(canonical.get_account(&[2u8; 32]).unwrap().balance, 50);
    assert_eq!(canonical.get_account(&[1u8; 32]).unwrap().nonce, 0);
}

#[test]
fn successful_snapshot_execution_commits_changes() {
    let canonical = canonical_state();
    let mut snapshot = StateSnapshot::from_canonical(&canonical);

    snapshot
        .apply_signed_transaction(signed_tx([1u8; 32], [2u8; 32], 10, 0))
        .unwrap();

    let new_canonical = snapshot.into_state();

    assert_eq!(new_canonical.get_account(&[1u8; 32]).unwrap().balance, 89);
    assert_eq!(new_canonical.get_account(&[2u8; 32]).unwrap().balance, 60);
    assert_eq!(new_canonical.get_account(&[1u8; 32]).unwrap().nonce, 1);
    assert_eq!(
        new_canonical.get_account(&FEE_COLLECTOR).unwrap().balance,
        1
    );

    // The old canonical binding is a distinct value and was never touched.
    assert_eq!(canonical.get_account(&[1u8; 32]).unwrap().balance, 100);
}
