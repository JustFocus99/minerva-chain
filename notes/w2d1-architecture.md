# Day 01 — Architecture Notes

## Core concepts

### Account
An account is the basic unit of state in an account-based system. It represents an identity that can hold funds and participate in execution.

### Balance
A balance is the amount of value owned by an account. It is updated by transfers, minting, and other state transitions.

### Nonce
A nonce is a counter attached to an account. It helps track the order of transactions from that account and prevents replay.

### Transaction
A transaction is a signed request to change state. It usually contains the sender, receiver, amount, nonce, and a signature.

### Signature
A signature proves that the transaction was authorized by the sender's private key. It protects against tampering and forgery.

### State transition function
A state transition function takes an old state and a transaction or block and returns a new state. It defines the rules of execution.

### State commitment
A state commitment is a deterministic digest of the state. It allows systems to summarize and verify state without needing to inspect every field.

### Block
A block is a bundle of transactions plus metadata such as parent hash, height, and block header fields. It is the unit of progression in the chain.

### Parent hash
A parent hash links a block to its predecessor. It creates a chain and makes tampering with history visible.

### Replay protection
Replay protection prevents the same valid transaction from being accepted multiple times as if it were new. Nonces and chain context are common mechanisms.

## Why does an account need a nonce?
An account needs a nonce so that transactions can be ordered and so the system can reject duplicates or old transactions. Without a nonce, a signed transaction could be replayed again and again, causing repeated state changes.

## Why must execution be deterministic?
Execution must be deterministic so that every validator or executor reaches the same result from the same starting state and inputs. If execution depends on nondeterministic factors, different nodes could disagree about the state.

## Why is a block more than a list of transactions?
A block is more than a list of transactions because it also carries structure and linkage. It includes ordering rules, metadata, a parent reference, and a commitment that ties the block into the chain history.

## Why should invalid transactions not mutate state?
Invalid transactions should not mutate state because execution must be atomic and safe. If a transaction fails validation, the system should leave the state unchanged rather than partially applying effects.

## What is the difference between transaction validity and block validity?
Transaction validity asks whether a single transaction is acceptable under the current state and rules. Block validity asks whether a full block is structurally and semantically correct, including parent linkage, ordering, header contents, and the resulting state transition.

## Why does parent hash linking matter?
Parent hash linking matters because it makes the chain history explicit and tamper-evident. If someone changes an earlier block, the hash chain no longer matches and the inconsistency becomes detectable.
