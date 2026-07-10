# Day 06 — Week 2 state audit

## Current Week 2 status

Week 2 implemented deterministic single-block execution on top of an in-memory `ChainState`. The pieces that exist today, read directly from `crates/transaction`, `crates/state`, and `crates/block`:

- `UnsignedTransaction { from, to, amount, nonce }` with a `to_bytes()` encoding and an `id()` derived by hashing that encoding (`crates/transaction/src/transaction.rs`).
- `SignedTransaction { transaction, public_key, signature }` with `sign()` and `verify()`.
- `ChainState` backed by `BTreeMap<AccountId, Account>` (`crates/state/src/chain_state.rs`), so account iteration and `state_commitment()` are already deterministic.
- `apply_signed_transaction` — verifies signature, checks sender/receiver exist, rejects zero amount and self-transfer, requires `tx.nonce == sender.nonce` exactly, checks balance, then mutates a cloned state.
- `execute_block` — recomputes the Merkle root over transaction IDs and rejects the block if it does not match `header.transaction_root`; executes transactions in the block's given order against a clone of the parent state; rejects the block if the resulting `state_commitment()` does not match `header.state_commitment`.
- `Block` / `BlockHeader` with `height`, `parent_hash`, `transaction_root`, `state_commitment`, `producer`, `slot`, and a `compute_hash()`.

## What works

- Deterministic state commitment: because `ChainState` already uses `BTreeMap`, `state_commitment()` and `total_supply()` do not depend on hash iteration order.
- Atomic execution: both `apply_signed_transaction` and `execute_block` operate on a cloned state and only return the new state on success, so a rejected transaction or block never leaves partial mutations visible (per the Week 2 day-04 note).
- Merkle root and state commitment are both checked as part of block execution, so a block that lies about its contents or its resulting state is rejected before it can be adopted.
- Balance and nonce arithmetic for accounts goes through `checked_add_amount` / `checked_sub_amount` in `primitives::amount`, which return `PrimitiveError` instead of panicking on overflow/underflow (`Account::deposit` / `Account::withdraw` use this; the transfer path in `apply_signed_transaction` uses raw `-`/`+` on already-checked balances since the balance check happens first).

## What is missing

- **No mempool crate.** There is no admission layer at all yet — `execute_block` is handed a `Vec<SignedTransaction>` directly with no notion of a pending pool, no duplicate-ID check across the pool, and no nonce-gap handling. `execute_block` also does not itself reject duplicate transaction IDs within a single block; two transactions with the same ID would simply be applied twice if they both passed per-transaction checks.
- **No fee field.** `UnsignedTransaction` has no `fee`. Week 3 fee accounting needs a design decision on whether fee is a separate field or derived, and how it interacts with `checked_sub_amount`.
- **Nonce checking is strict-equality, not gap-aware.** `apply_signed_transaction` requires `tx.nonce == sender.nonce`. That is correct for *execution*, but a mempool needs to hold future-nonce transactions rather than reject them outright — this is a new concept, not something Week 2 provides.
- **Signature scheme is a placeholder.** `crypto::signature::sign_message` / `verify_signature` (`crates/crypto/src/signature.rs`) use a fixed `DEFAULT_PUBLIC_KEY` and a deterministic hash-based "signature" — `verify()` only proves the message matches a fixed keypair, not that `public_key` corresponds to `tx.from`. Any mempool admission rule that assumes "valid signature implies authorized by `from`" is currently unsound; this is worth flagging rather than fixing today since Week 3's mandate is pool/storage/replay, not cryptography.
- **No CLI, storage, replay, or fork choice** — none of that exists yet; today's scope is pool design only.

## What must not change in Week 3

- `ChainState` must remain keyed by `BTreeMap`, not `HashMap`. Any new pool or index structure follows the same rule (see design decision in [w3d1-mempool.md](w3d1-mempool.md)).
- `execute_block`'s check order — verify `transaction_root`, execute transactions against a cloned state, verify `state_commitment` — must not be reordered or weakened. The mempool is a pre-filter; it does not replace block-level validation.
- Atomicity of `apply_signed_transaction` / `execute_block`: a rejected transaction or block must leave the input state untouched. The mempool must not bypass this by mutating `ChainState` directly.
- A replayed block must still reproduce the exact same `state_commitment`. Nothing added to the mempool layer may introduce non-determinism (wall-clock time, random tie-breaking, `HashMap` iteration) that could change which transactions end up in a block or in what order.

## What invariants must remain true

- Signature verification happens before any state mutation.
- Invalid transactions never mutate `ChainState` (enforced today by clone-then-commit in `apply_signed_transaction`).
- A block's `transaction_root` must match the Merkle root of its actual transactions, and its `state_commitment` must match the state produced by executing it.
- Total supply is conserved across a valid transfer (`sender_new_balance + receiver_new_balance` unchanged in aggregate) — this will need revisiting once fees are introduced, since a fee changes where value goes rather than whether it's conserved.
