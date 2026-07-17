//! Day 5 Hour 2 -- fuzzes `storage::record::decode_signed_transaction`,
//! the fixed-176-byte transaction wire format used inside a block's
//! payload (see `docs/storage.md` and `record.rs`'s
//! `TRANSACTION_RECORD_LEN`).
//!
//! Fuzz rule (see `docs/fuzzing.md`):
//!   - arbitrary bytes may return `Err`
//!   - arbitrary bytes must never panic or produce undefined behavior
//!   - a decoded value must not be treated as "valid" beyond what decoding
//!     actually checked -- this is a *structural* decode only, not
//!     signature or balance validation (that's `SignedTransaction::verify`
//!     / `ChainState::apply_signed_transaction`'s job, exercised here only
//!     to confirm it never panics on decoder output, garbage or not)

#![no_main]

use libfuzzer_sys::fuzz_target;
use storage::record::{decode_signed_transaction, encode_signed_transaction};

fuzz_target!(|data: &[u8]| {
    let Ok(tx) = decode_signed_transaction(data) else {
        return;
    };

    // A decoder that reports success must have consumed exactly `data` and
    // nothing else: re-encoding the decoded value must reproduce the
    // original bytes exactly. Any mismatch here would mean the decoder
    // silently dropped, reordered, or misread part of a "successfully"
    // decoded transaction.
    assert_eq!(encode_signed_transaction(&tx), data);

    // Decoding never implies validity -- a garbage-but-structurally-sized
    // input decodes into a `SignedTransaction` whose signature essentially
    // never verifies. `verify` must still handle that cleanly (no panic),
    // since nothing upstream of it filters decoder output before this
    // point in the real pipeline.
    let _ = tx.verify();
});
