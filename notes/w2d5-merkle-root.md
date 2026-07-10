# Day 05 — Merkle root

A Merkle root is a single hash that commits to all transactions in a block.

If any transaction changes, its leaf hash changes, then parent hashes change, and finally the root changes.

If the transaction order changes, the hashing structure changes, so the Merkle root changes too.

The block header should store a transaction_root so the block can commit to the exact transaction set without storing every transaction in the header.

If a block's transaction_root does not match its transactions, the block should be considered invalid.

Deterministic ordering is important because every validator should compute the same Merkle root for the same transaction list.
