# Invariants

## Invariant: Total token supply cannot change except through an explicit mint path.

Why this matters:
The total supply is a core property of the ledger and should not drift silently through ordinary transfers or execution errors.

How the code will enforce it:
Supply changes will be allowed only through a dedicated mint or issuance path, and ordinary transfer logic will not modify the total supply.

What test should prove it:
Create a transfer transaction and assert that the total supply remains unchanged. Also test a mint path separately and assert that the supply changes only there.

## Invariant: Sender balance cannot become negative.

Why this matters:
A sender must never be able to spend more than they own.

How the code will enforce it:
Execution will check that the sender balance is at least the transfer amount before applying the transaction.

What test should prove it:
Create a transaction from an account with insufficient balance, execute it, and assert the execution fails and the balances remain unchanged.

## Invariant: Nonce must increase monotonically.

Why this matters:
Nonces prevent replay and help preserve transaction ordering semantics.

How the code will enforce it:
Each transaction must carry a nonce, and execution will reject transactions whose nonce is not greater than the sender's current nonce.

What test should prove it:
Submit a transaction with a nonce lower than or equal to the current nonce and assert that execution fails.

## Invariant: Invalid signatures cannot mutate state.

Why this matters:
A malicious user must not be able to alter balances by submitting forged transactions.

How the code will enforce it:
Signature verification happens before any account mutation.

What test should prove it:
Create a valid transaction, modify the amount after signing, execute it, assert execution fails, and assert both sender and receiver balances remain unchanged.

## Invariant: Failed transactions cannot partially modify state.

Why this matters:
Execution should be atomic. A failed transaction should leave the state exactly as it was before.

How the code will enforce it:
State changes will be applied only after all preconditions succeed, and any failure will abort the transaction before any mutation is committed.

What test should prove it:
Construct a transaction that fails after validation starts, execute it, and assert no balance or nonce changes occurred.

## Invariant: Replaying the same block from the same parent state must produce the same state commitment.

Why this matters:
Deterministic execution is essential for reproducible validation and debugging.

How the code will enforce it:
Block execution must be deterministic and use only the parent state, block contents, and explicit rules to compute the next state and commitment.

What test should prove it:
Execute the same block twice from the same parent state and assert that both resulting state commitments are identical.

## Invariant: Block parent hash must match expected parent.

Why this matters:
A block must connect to the intended chain history.

How the code will enforce it:
Block validation will compare the declared parent hash against the hash of the referenced parent block or parent state commitment.

What test should prove it:
Create a block with a mismatched parent hash, attempt to validate it, and assert that validation fails.

## Invariant: Block height must follow parent height.

Why this matters:
Height is part of the chain's structural consistency and should not jump unexpectedly.

How the code will enforce it:
Validation will require that a block's height is exactly one greater than its parent height.

What test should prove it:
Create a child block with an incorrect height and assert that validation fails.

## Invariant: Transaction ordering inside a block must be deterministic.

Why this matters:
Different ordering of the same transactions could lead to different outcomes or confusion about execution semantics.

How the code will enforce it:
The block execution logic will use a defined order for transactions, rather than relying on incidental ordering.

What test should prove it:
Construct a block with the same transactions in a different order and assert that the implementation handles them according to the documented deterministic order.

## Invariant: Block hash must change when header or transaction content changes.

Why this matters:
The block hash should reflect the exact contents of the block and should not be stable across unrelated changes.

How the code will enforce it:
Block hashing will include the relevant header fields and transaction contents in a deterministic way.

What test should prove it:
Modify a header field or transaction content and assert that the resulting block hash changes.
