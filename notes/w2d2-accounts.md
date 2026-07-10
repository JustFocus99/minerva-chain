# Day 02 — Solana account concepts

## Core concepts

### Account
In Solana, an account is a data container owned by a program. It can hold lamports, data, and metadata such as the owner program.

### Lamports
Lamports are the smallest unit of SOL. They are the native balance attached to an account.

### Owner
The owner field identifies which program is authorized to modify the account's data. This is a key separation between accounts and programs.

### Instruction
An instruction is a request telling a program to do something. It usually contains the program id, accounts it will touch, and instruction-specific data.

### Transaction
A transaction is a bundle of instructions. It is signed by one or more signers and submitted by a client.

### Signer
A signer is an account whose private key authorizes a transaction or instruction. Signers are required for state changes that should be authenticated.

## How is my Account model similar to a Solana account?
Both models treat an account as a stateful entity that can hold value and participate in state transitions. In both cases, the account has an identity and its own balance-like state.

## How is it different?
My model is much simpler. It focuses on a basic balance and nonce, while Solana accounts also carry ownership, executable status, data storage, rent rules, and program-specific semantics.

## What am I intentionally simplifying?
I am simplifying away program-owned account metadata, arbitrary data blobs, runtime execution semantics, rent, account size rules, and the full Solana transaction/instruction architecture.

## Why does Solana separate accounts from programs?
Solana separates accounts from programs so that state can live independently of the code that manages it. Programs are executable logic, while accounts are persistent storage and ownership boundaries. This separation enables composability, permissioning, and state reuse across many transactions.
