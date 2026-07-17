//! Day 5 Hour 3 -- unlike `block_decoder`/`transaction_decoder` (which stop
//! at "does the decoder panic"), this target chains the *whole* untrusted
//! path together: raw bytes -> `storage::record` decoders -> execution
//! against a real `ChainState`. `docs/fuzzing.md`'s "Non-goals" section
//! notes `ChainState::execute_block` is normally only ever fed in-process,
//! Rust-typed values that already passed decoding -- this target is the
//! exception, there specifically to prove that a `Block` or
//! `SignedTransaction` that *did* survive decoding (i.e. is
//! structurally well-formed, see `record.rs`) can't panic execution no
//! matter how semantically bogus it is (bad signature, unknown sender,
//! wrong nonce, replayed tx, ...). Execution rejecting it with an `Err` is
//! the expected, correct outcome for almost every generated input.
//!
//! Fuzz rule (see `docs/fuzzing.md`):
//!   - `Err` is fine (from decoding *or* execution)
//!   - a panic anywhere in the chain is not fine
//!   - execution accepting a block/transaction it shouldn't is not fine --
//!     covered by the existing in-process tests
//!     (`state::chain_state::execute_block`'s own test suite), not
//!     re-asserted here, since this target's job is "does it crash", not
//!     "is validation correct"

#![no_main]

use libfuzzer_sys::fuzz_target;
use primitives::AccountId;
use state::account::Account;
use state::chain_state::ChainState;
use storage::record::{decode_record, decode_signed_transaction};

const ALICE: AccountId = [1u8; 32];
const BOB: AccountId = [2u8; 32];
const FEE_COLLECTOR: AccountId = [3u8; 32];

/// A small, fixed chain state to execute fuzzer-decoded values against --
/// two funded accounts and a registered fee collector, no tip yet (so the
/// genesis convention from `docs/block-validation.md` applies to any
/// decoded block).
fn test_state() -> ChainState {
    let mut state = ChainState::new();
    state.create_account(Account::new(ALICE, 1_000_000));
    state.create_account(Account::new(BOB, 1_000_000));
    state.create_account(Account::new(FEE_COLLECTOR, 0));
    state.set_fee_collector(FEE_COLLECTOR);
    state
}

fuzz_target!(|data: &[u8]| {
    // Path 1: decode a whole block record and execute it against a fresh
    // chain state, exactly as `AppendOnlyBlockStore::load_blocks` output
    // would eventually reach `ChainState::execute_block` via replay.
    if let Ok((block, _record_len)) = decode_record(data, 0) {
        let parent = test_state();
        let _ = ChainState::execute_block(&parent, block);
    }

    // Path 2: decode a single signed transaction and apply it directly,
    // the same call `ChainState::execute_block` makes per-transaction
    // inside a block.
    if let Ok(tx) = decode_signed_transaction(data) {
        let mut state = test_state();
        let _ = state.apply_signed_transaction(tx);
    }
});
