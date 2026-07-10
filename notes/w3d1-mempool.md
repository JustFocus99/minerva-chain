# Day 01 — Transaction pool, nonce ordering, and fee design

## What is a mempool?

A mempool is a node's temporary waiting area for transactions that are valid enough to be considered, but not yet included in a block.

```text
User creates transaction
        ↓
Node receives transaction
        ↓
Node checks basic validity
        ↓
If valid, transaction enters mempool
        ↓
Block producer selects transactions from mempool
        ↓
Transactions are executed inside a block
        ↓
Block becomes part of the chain
        ↓
Included transactions are removed from mempool
```

For minerva-chain, the mempool is not shared across a network — it is only local memory inside a single node.

Core definition:

```text
A mempool is a temporary transaction pool.

It stores transactions that have been submitted to the node but have not yet been included in a block.

The mempool does not finalize transactions.
The mempool does not change account balances.
The mempool does not decide the canonical chain.

It only decides whether a transaction is allowed to wait for future block production.
```

Mental model:

```text
State execution changes balances.
Block import changes the chain.
Mempool only stores waiting transactions.
```

So if Alice sends Bob 10 coins and the transaction enters the mempool, Bob does not receive the money yet. Bob receives the money only after the transaction is included in a valid block and that block is imported.

### Mempool responsibilities

- reject invalid signatures
- reject duplicate transaction IDs
- reject stale nonces
- hold future-nonce transactions
- expose ready transactions in deterministic order
- remove transactions after they are included in a block

### Mempool non-responsibilities

- it does not finalize transactions
- it does not change balances
- it does not choose the canonical chain
- it does not replace block validation
- it does not trust its own previous checks during block import

The mempool performs **admission checks**. The block executor performs **state transition**. It should not accept garbage, but it should also not fully execute transactions like a block.

Immediate-rejection cases: invalid signature, duplicate transaction ID, malformed transaction bytes, stale nonce, fee overflow, sender account does not exist, transaction too large (if a size limit is defined).

## Nonce behavior

Nonce means: the expected transaction number for an account.

```text
Current account nonce = N

Transaction nonce == N → ready
Transaction nonce > N  → pending/future
Transaction nonce < N  → stale/reject
```

Example — Alice current nonce = 2:

```text
tx nonce 2 → can be included next
tx nonce 3 → wait until nonce 2 executes
tx nonce 5 → wait, but there is a gap
tx nonce 1 → reject as stale
```

This is why the mempool needs ordering — it cannot randomly choose Alice's nonce 5 transaction before Alice's nonce 2 transaction.

### Ready vs pending

```text
Ready transaction:
A transaction that can be included in the next block.

Pending transaction:
A transaction that might become valid later, usually because its nonce is too high.
```

Example — Alice current nonce = 0:

```text
Alice tx nonce 0 → ready
Alice tx nonce 1 → pending
Alice tx nonce 2 → pending
```

After nonce 0 executes in a block:

```text
Alice current nonce = 1
Alice tx nonce 1 becomes ready
Alice tx nonce 2 stays pending
```

Rule summary:

- **Ready**: nonce equals the sender account's current nonce.
- **Pending**: nonce is greater than the sender account's current nonce.
- **Rejected**: nonce is lower than the sender account's current nonce, signature is invalid, or transaction ID already exists in the pool.

## Simple Week 3 API plan

```text
submit_transaction()
remove_included_transactions()
ready_transactions()
pending_transactions()
pool_size()
contains_transaction_id()
```

Suggested internal design:

```rust
BTreeMap<AccountId, BTreeMap<Nonce, SignedTransaction>>
```

`BTreeMap` gives deterministic ordering — two nodes with the same mempool contents must produce the same transaction order. Ordering rule: sort by sender `AccountId`, then by nonce, then by transaction ID if needed. This matches the deterministic-ordering design decision below and the existing `ChainState` convention (`BTreeMap<AccountId, Account>`).

## Week 3 new invariants

- Duplicate transaction IDs are rejected — both within the mempool and within a single block.
- Invalid signatures never enter the pool.
- Nonce gaps are held, not executed — a transaction whose nonce is ahead of the account's current nonce waits in the pool instead of being dropped or applied out of order.
- A transaction can be included at most once (pool admission and block inclusion must agree on this — the pool should not hand out the same transaction ID twice).
- Fees are calculated with checked arithmetic (reuse `checked_add_amount` / `checked_sub_amount` from `primitives::amount`, the same pattern already used by `Account::deposit` / `Account::withdraw`).
- Transaction ordering must be deterministic — for the same set of pool contents, two runs (or two nodes) must produce the same candidate ordering for block building.
- Mempool order must not depend on `HashMap` iteration order.

### Design decision: deterministic ordering

Use deterministic ordering. Prefer:

```rust
BTreeMap
BTreeSet
Vec sorted by explicit key
```

Avoid relying on:

```rust
HashMap iteration order
HashSet iteration order
```

because they are not stable ordering mechanisms. `ChainState` already follows this rule via `BTreeMap<AccountId, Account>` — the mempool should follow the same convention rather than introducing a `HashMap`-based index and sorting it ad hoc at read time.

## Implementation retrospective

The plan above was written before any of it existed. This section is what actually happened building it, hour by hour.

### What I learned about mempools

A mempool is smaller in scope than it sounds. It doesn't execute anything — it only answers "is this transaction allowed to wait?" The real complexity isn't the happy path, it's the ordering: nonces force a sender's transactions into a strict sequence, so the pool has to think in terms of *ready* (nonce matches account state) versus *pending* (nonce is ahead) rather than a flat "valid/invalid" split. Admission and execution also have to independently re-check the same invariants (signature, nonce, balance, fee) — the mempool's opinion is never trusted at block-execution time, on purpose, because the state can move between admission and inclusion.

### What I implemented

- `TransactionPool` storing transactions as `BTreeMap<AccountId, BTreeMap<Nonce, SignedTransaction>>`, plus a `BTreeSet<TransactionId>` for O(log n) duplicate-ID lookups.
- `submit_transaction`, which runs every admission check before mutating anything: duplicate ID → malformed body → invalid signature → sender missing → stale nonce → duplicate nonce for sender → fee overflow → insufficient fee balance → insert.
- `PoolAdmission` as four distinct outcomes (`Accepted`, `QueuedForFutureNonce`, `Duplicate`, `Rejected(TransactionPoolError)`) rather than a plain `Result`, since "accepted but not ready yet" and "rejected outright" are semantically different and both needed to be visible to callers.
- `ready_transactions` / `pending_transactions` / `ordered_transactions`, all derived by walking the nested `BTreeMap`s, so ordering falls out of the data structure instead of being sorted at read time.
- On the `state` side: a fee collector account on `ChainState`, `BASE_FEE` in `primitives`, and checked-arithmetic fee debiting in `apply_signed_transaction` (see `docs/fee-model.md`).

### What validation the pool performs

At admission, in order: transaction ID not already seen; `UnsignedTransaction::is_valid()` (amount > 0, from != to); signature verifies against the transaction bytes; sender account exists in the given `ChainState`; nonce is not behind the sender's current nonce; no other transaction already occupies that sender+nonce slot; `amount + BASE_FEE` doesn't overflow; sender's balance covers `amount + BASE_FEE`.

### What the pool does not do

It does not execute transactions, move balances, or increment nonces — that's `ChainState::apply_signed_transaction`'s job. It does not choose which block a transaction lands in. It does not guarantee a transaction it accepted will still be valid by the time a block producer picks it up (a nonce gap can be filled, or a balance can drop below what's needed, in between). It does not currently evict transactions once their block is imported, or enforce a pool size limit, or expire old pending transactions.

### How nonce ordering works

Every sender's transactions are keyed by nonce in their own `BTreeMap<Nonce, SignedTransaction>`, so they're already sorted per sender. A submitted nonce is compared against the sender's *current on-chain* nonce (from `ChainState`, not from the pool): lower is stale and rejected outright; equal is `Accepted` and immediately ready; higher is `QueuedForFutureNonce` — stored, but excluded from `ready_transactions` until the account's nonce catches up. `ready_transactions` only ever returns the single transaction sitting at exactly the account's current nonce per sender, never anything past a gap, so a gap can't be executed out of order.

### What tests were added

Hour 3: `rejects_duplicate_transaction`, `allows_different_transaction_ids`.
Hour 4: `rejects_invalid_signature_before_pool_insert`, `invalid_signature_does_not_change_pool_size`.
Hour 5: `accepts_expected_nonce`, `queues_future_nonce`, `rejects_stale_nonce`, `does_not_execute_nonce_gap`, `orders_transactions_by_sender_and_nonce_deterministically`, `rejects_duplicate_nonce_for_sender`.
Hour 6 (fee model, mostly on the `state` side): `successful_transaction_charges_fee`, `insufficient_fee_balance_rejects_transaction`, `fee_overflow_rejects_transaction`, `missing_fee_collector_rejects_transaction`.
Hour 7 (mempool-side fee admission + adversarial coverage): `rejects_insufficient_fee_balance`, `rejects_fee_overflow`, `rejects_transaction_with_malformed_bytes`, `rejects_duplicate_transactions`, `rejects_invalid_signatures`, `rejects_nonce_gap_from_ready_set`.

### What bugs/confusions I had

- **Borrow checker with multiple mutable account lookups.** Fetching `sender_account`, `receiver_account`, and `fee_collector_account` as separate `&mut Account` from the same `BTreeMap` and holding them alive across several statements doesn't compile — Rust won't allow overlapping mutable (or mutable + immutable) borrows of `self.accounts`, even for different keys. Fixed by reading everything as owned copies first, doing all the arithmetic, and only taking `get_account_mut` one at a time at the very end.
- **`?` doesn't cross error types for free.** `checked_add_amount` returns `Result<_, PrimitiveError>`, but `apply_signed_transaction` returns `Result<_, StateError>`. `?` only auto-converts if a `From<PrimitiveError> for StateError` impl exists; otherwise it's a compile error (or worse, a silently wrong `.map_err` to the wrong variant, which I did once before catching it).
- **Switching from `Vec<SignedTransaction>` to a nested `BTreeMap` silently changed what `len()` meant.** It briefly returned the number of distinct senders instead of the total transaction count, which passed `cargo build` fine and only broke a test that happened to use two transactions from the same sender.
- **The fee-overflow check started as a placeholder.** Before `BASE_FEE` existed, "fee overflow" was implemented as `checked_add_amount(amount, 0)` — technically checked arithmetic, but not actually validating a fee. It only became a real check once the fee model and `BASE_FEE` were defined in Hour 6.
- **Two similarly-named error variants, only one wired up.** `StateError::InsufficientFeeBalance` and `StateError::InsufficientBalance` both exist; only `InsufficientBalance` is ever constructed, since its `required` field is already `amount + fee`. `InsufficientFeeBalance` on the `state` side is currently dead — it's `mempool::TransactionPoolError::InsufficientFeeBalance` that does real work, at admission time.
- **Adding the fee collector requirement broke tests that predated it.** Every existing test that called `apply_signed_transaction(...).unwrap()` without registering a fee collector started panicking with `FeeCollectorMissing`, and balance literals that predated the fee needed updating (e.g. a sender ending up at 74 instead of 75 once a 1-unit fee was also being charged). A reminder that adding an invariant to a shared code path has to be paired with an audit of every caller, not just the new tests.
