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
