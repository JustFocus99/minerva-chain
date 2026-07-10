# Day 03 — Solana transaction structure

## Core structure

A Solana transaction is built from a few essential pieces:

- Message: the canonical payload that describes the intended state change.
- Account keys: the accounts that will be involved in the transaction.
- Recent blockhash: a recent hash that ties the transaction to a specific slot and helps prevent replay across time.
- Signatures: one or more digital signatures proving that the listed signers authorized the message.
- Instructions: the requested operations to execute, such as transferring lamports or invoking a program.

## What is the message?

The message is the part that gets serialized and signed. In practice, it is the canonical byte representation of the transaction intent. The message includes the account keys, the recent blockhash, and the instructions that define what should happen.

## What does Solana sign?

Solana signs the serialized message bytes, not a loosely structured object. Each signer creates a signature over the canonical message bytes. The signature is then attached to the transaction so anyone can verify that the message was authorized by the matching private key.

## Why is deterministic message serialization mandatory?

Deterministic serialization is mandatory because every validator and client must derive the exact same bytes for the same logical transaction. If serialization is not canonical, two implementations could produce different byte sequences for the same transaction, causing:

- signature verification failures,
- inconsistent transaction IDs,
- ambiguity in replay protection,
- different results across clients and validators.

A canonical encoding makes the transaction stable, reproducible, and portable.

## Why does changing one field invalidate a signature?

A signature is created over the full serialized message. If any field changes, the bytes change, so the signature no longer matches the recomputed signature over the new bytes. This is the core security property of digital signatures: they bind the signer to the exact content that was signed.

## How does this relate to my model?

My model is similar in one important way: a transaction is an intent to move state, and that intent is signed so the state transition can be authenticated. The current model also uses deterministic serialization and signature verification over a canonical byte representation.

## What is simplified in my model?

My model is intentionally simpler than Solana:

- It has a small transaction shape with sender, receiver, amount, and nonce.
- It does not model account keys as a full account-indexed message structure.
- It does not include recent blockhashes, multiple instructions, program invocation, or runtime execution.
- It does not support multi-signature, address tables, or complex transaction fees.

## Takeaway

Solana's transaction design is centered on a canonical message, explicit account participation, and signatures over that exact message. My model keeps the same core idea—signed, deterministic state transitions—but strips away the broader runtime and networking complexity.
