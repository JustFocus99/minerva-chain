# Minerva Chain

minerva-chain is an educational Rust blockchain execution prototype.
It focuses on account-based state, signed transactions, deterministic execution,
block validation, state commitments, and adversarial testing.
It is not a production blockchain, not a consensus protocol, and not a cryptocurrency.

## Purpose

This repository is meant to explore how a simple blockchain execution engine might be structured in Rust.
It is intended for learning, discussion, and experimentation rather than deployment.

## Current status

The project is in an early stage. The repository contains notes, architecture documents, and a small Rust codebase that is still being shaped.

## Design goals

- Keep the implementation understandable and explicit.
- Model core execution concepts in a minimal way.
- Favor deterministic behavior where possible.
- Use tests to document assumptions and guard against regressions.

## Non-goals

- Building a production-ready blockchain.
- Implementing a consensus protocol.
- Creating a cryptocurrency or token economy.
- Supporting real-world network deployment.

## Repository structure

- crates/ - core Rust crates for the system.
- docs/ - architecture and invariant notes.
- notes/ - working notes from the development process.
- tests/ - test cases and integration checks.

## How to run tests

Run the test suite from the repository root with:

```bash
cargo test
```

## Limitations

This is a prototype. It is not intended to be secure, performant, or complete enough for production use.
It should be treated as an educational artifact and a place to reason about execution design.

