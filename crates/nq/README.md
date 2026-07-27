# nq

`nq` is the decision and evaluation component of the NQ constellation. It
owns bounded refusal vocabulary, evaluation dispositions, supporting evidence
identities, and consumer-indexed reliance decisions.

The crate is deliberately clock-free, storage-free, and transport-free.
Callers provide an already-evaluated receipt through the narrow
`EvaluatedReceipt` contract. NQ decides whether a configured consumer may rely
on that evaluation for a declared purpose; it does not collect evidence,
schedule checks, mutate monitored systems, deliver notifications, or render a
dashboard.

## Authority boundary

A reliance authorization is a decision input, not execution authority. Every
reliance receipt states that it grants no capability, action, retry, clearing,
or escalation permission. A refusal is not a refutation of the underlying
claim.
