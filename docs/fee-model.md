# Fee Model

## Purpose

This document defines the fee rules for minerva-chain. It is a design
document for Week 3 — the mempool and execution layers should be built to
match these rules, not the other way around.

## Rules

- Every transaction pays a fixed base fee: `primitives::BASE_FEE` (currently
  `1`).
- The sender is the fee payer.
- The fee is charged only when the transaction is included in a valid block.
- A transaction rejected by the mempool pays no fee.
- A transaction rejected during block validation pays no fee, because the
  whole candidate block is discarded.
- The fee collector is a special account.
- The sender must have enough balance for `amount + fee`.
- All fee and transfer math uses checked arithmetic.
- If fee calculation overflows, the transaction is rejected.
- If the sender cannot pay `amount + fee`, the transaction is rejected.
- Fee debit and transfer execution are atomic.

## Implementation status

These rules are enforced at two layers, and both layers check them
independently rather than trusting each other (see
`notes/w3d1-mempool.md` — "it does not trust its own previous checks during
block import"):

- **Mempool admission** (`mempool::pool::TransactionPool::submit_transaction`)
  rejects a transaction up front if `amount + BASE_FEE` overflows
  (`TransactionPoolError::FeeOverflow`) or the sender's current balance can't
  cover it (`TransactionPoolError::InsufficientFeeBalance`). Nothing is
  mutated at this layer — admission never touches a balance.
- **Block execution** (`state::chain_state::ChainState::apply_signed_transaction`)
  re-derives `total_debit = amount + BASE_FEE`, re-checks the sender's
  balance, and actually moves the fee to the fee collector. It does not
  assume the mempool already validated this.

## When the fee is charged

Fee-worthiness and inclusion are two different things:

```text
Mempool admission  → transaction is allowed to wait, no balance is touched
Block validation   → candidate block is checked as a whole
Block execution    → included transactions run, fee is charged here
```

The mempool only decides whether a transaction is allowed to wait. It never
charges a fee, because it never mutates balances. A transaction only pays a
fee once it is executed as part of a block that is accepted onto the chain.
If a candidate block fails validation, the entire block — and every fee it
would have charged — is discarded along with it. There is no partial
charging of fees for transactions inside a rejected block.

## When the fee is not charged

No fee is charged in any of these cases, and in every case no balance moves
at all:

- The transaction never makes it into the mempool (invalid signature, stale
  nonce, duplicate transaction ID, duplicate nonce for the sender, malformed
  transaction body, `amount + fee` overflow, or the sender can't afford
  `amount + fee`).
- The transaction sits in the mempool as pending (nonce ahead of the
  account's current nonce) and never gets included.
- The candidate block containing the transaction fails validation (bad
  transaction root, bad state commitment, bad parent hash) and is discarded
  whole.
- Execution of the transaction itself fails during block execution
  (`StateError::InvalidSignature`, `SenderMissing`, `ReceiverMissing`,
  `ZeroAmount`, `SenderEqualsReceiver`, `InvalidNonce`,
  `InsufficientBalance`, `Amount` (overflow), or `FeeCollectorMissing`) —
  since execution aborts before any balance is touched.

## Balance requirement

A transaction is only payable if the sender's balance covers both the
transfer amount and the fee:

```text
required = amount + fee
sender.balance >= required
```

If `sender.balance < required`, the transaction fails and no balances move.
This check happens before any mutation, not after. It is enforced twice:
`TransactionPoolError::InsufficientFeeBalance` at mempool admission, and
`StateError::InsufficientBalance { available, required }` at block
execution. (`StateError::InsufficientFeeBalance` exists as a placeholder
variant but is currently unused — `InsufficientBalance`'s `required` field
is already `amount + fee`, so it covers this case without a separate
variant.)

## Atomicity

Atomic means:

```text
either all balance changes happen
or none happen
```

A transaction moves two amounts out of the sender's balance: the transfer
amount (to the receiver) and the fee (to the fee collector). Both movements
must succeed together or not at all. There is no state where the sender has
paid the fee but the transfer didn't happen, or paid the transfer but not
the fee.

### Failure example

```text
Alice balance = 10
Alice sends Bob 10
fee = 1
total needed = 11
```

Alice's balance (10) is less than the required amount (11), so the
transaction fails before anything is applied.

Correct result:

```text
Alice still has 10
Bob receives 0
Fee collector receives 0
transaction fails
```

Wrong result (partial mutation, must never happen):

```text
Alice loses fee
Bob receives nothing
```

## Overflow handling

All fee and transfer arithmetic uses checked arithmetic (`checked_add_amount`
/ `checked_sub_amount`), the same pattern already used by `Account::deposit`
and `Account::withdraw`. If computing `amount + fee` overflows, the
transaction is rejected before any balance is touched — it never wraps or
saturates silently. At mempool admission this is
`TransactionPoolError::FeeOverflow`; at block execution it surfaces as
`StateError::Amount(PrimitiveError::AmountOverflow)`, via a
`From<PrimitiveError> for StateError` conversion so the `?` operator can
propagate it directly out of `apply_signed_transaction`.

## Fee collector

The fee collector is a regular account, distinguished only by its role: fees
are credited to it instead of to a transaction's declared receiver. It is
subject to the same checked-arithmetic rules as any other account balance.

## Non-goals for Week 3

- Dynamic or market-based fee pricing.
- Fee refunds or partial fee charging.
- Multiple fee tiers or per-transaction-type fees.

These may be revisited in a later milestone, but Week 3 uses a single fixed
base fee for every transaction.
