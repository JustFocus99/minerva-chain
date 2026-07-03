# Day 04 — Execution and state mutation

## Why should validation happen before mutation?

Validation should happen before mutation because a transaction or transition should be checked while the current state is still intact. If validation is mixed with mutation, the system can leave behind partial changes when a rule is violated. This makes failures harder to reason about and can produce inconsistent state.

In blockchain-style systems, the rule is usually: verify first, then apply. That ensures the state transition is either fully accepted or fully rejected.

## When is cloning candidate state acceptable?

Cloning candidate state is acceptable when the system wants to explore or test a transition without touching the real state yet. It is especially useful for:

- validating a batch of transactions,
- simulating execution,
- comparing outcomes before commit,
- keeping the canonical state immutable during checks.

A clone is a safe way to reason about the effects of a proposed change before deciding whether to commit it.

## When is in-place mutation dangerous?

In-place mutation is dangerous when the system may need to abort the transition after changing state. If state is modified first and a later validation step fails, the system can end up with a partially applied change. This is risky when:

- multiple transitions are being processed together,
- a later transition depends on earlier state,
- validation is non-trivial or may fail for some accounts,
- the system must preserve atomicity.

This is especially dangerous in protocol software where a single bad transition should not corrupt the global state.

## What does atomic execution mean?

Atomic execution means that a transition either completes fully or has no visible effect at all. There is no halfway state. If the execution succeeds, all intended state changes are applied. If it fails, the state remains exactly as it was before the attempt.

In practice, this is often implemented by applying changes to a temporary copy and only committing them once all checks have passed.

## Why is this important in blockchain and protocol software?

It is important because protocol software must preserve correctness under failure, ambiguity, or adversarial input. Atomicity prevents inconsistent state, avoids double-application bugs, and makes execution easier to reason about. In blockchain systems, this matters even more because state is shared, deterministic, and often validated by many participants.

A protocol that does not enforce atomic execution can produce invalid state, broken invariants, or inconsistent histories that are hard to recover from.
