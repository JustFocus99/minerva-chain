# Architecture

## Purpose

This document describes the intended structure of the minerva-chain execution prototype.
The goal is to model a small account-based blockchain execution engine in Rust that can be reasoned about, tested, and extended.

## Non-goals

This project does not implement networking, proof of work, staking,
production validator consensus, token economics, or real wallet infrastructure.

## System model

The system is a deterministic state machine.
Given an initial state and a sequence of transactions or blocks, execution should produce a new state and a state commitment.
The model is intentionally small and explicit rather than full-featured.

## Account model

The ledger tracks accounts by address.
Each account has a balance and a nonce.
The prototype may also store other small metadata if needed for execution, but the main focus is account-based state updates.

## Transaction model

A transaction represents a signed state transition.
It should include at least:

- sender
- receiver
- amount
- nonce
- signature

The execution layer applies a transaction only if the signature is valid and the sender has enough balance.

## Signature model

Transactions are signed using a simple cryptographic signature scheme.
Signature verification is required before state changes are applied.
The signature is part of the transaction data and must be checked against the signed payload.

## Execution model

Execution is deterministic and should be free of hidden side effects.
A transaction is validated, then applied to the state if it succeeds.
If validation fails, the transaction should not mutate state.

The execution flow should be:

1. Parse and validate transaction structure.
2. Verify signature.
3. Check sender balance and nonce.
4. Apply state changes.
5. Return a success or failure result.

## Block model

A block bundles transactions and metadata.
A block should contain:

- parent hash
- height
- transactions
- header fields needed for hashing

Block execution should be deterministic given the same parent state and the same transaction list.

## State commitment model

The system should compute a state commitment for each resulting state.
This commitment is a deterministic digest of the relevant state and is used to reason about reproducibility and validation.
The exact encoding can remain simple at first, but it should be stable and deterministic.

## Determinism requirements

The same parent state, same block contents, and same execution rules must always produce the same resulting state and state commitment.
No randomness, wall-clock time, or non-deterministic data sources should affect execution.

## Failure handling

Execution should fail cleanly when invariants are violated.
Failures should not leave partial state changes behind.
The implementation should distinguish between validation failures and execution failures and make that behavior explicit in tests.

## Testing strategy

Tests should cover:

- successful transaction execution
- invalid signature rejection
- balance underflow prevention
- nonce enforcement
- block execution and commitment stability
- adversarial inputs that try to mutate state illegally

The tests should be written as executable examples of intended behavior and should be treated as part of the design.

## Future work

The next steps may include:

- richer account metadata
- clearer transaction and block serialization
- stronger commitment and hashing design
- more explicit state transition rules
- additional adversarial and property-based tests
