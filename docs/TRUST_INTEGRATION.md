# Trust Architecture → UBL Integration

This file maps the Trust Architecture concepts into UBL primitives.

The Trust Architecture document's core axioms include:
- **Data is never instructions** (prompt injection defense)
- **Operations are atomic and signed**
- **Identity is trajectory**
- **Behavior is observable**
- **Failure is bounded**
- **Multi-signature for high-value operations**

## What belongs in Chips/Programs vs Kernel

### 1) Isolation Barrier (Prompt Injection Defense)
**Best expressed as:** *Runtime service + Program inputs*, not a Chip.

Reason: the barrier is about **ingestion** — turning raw untrusted content into typed data. Chips should only read context, not parse arbitrary bytes.

Implementation in this repo:
- `POST /barrier/process` validates known schemas (invoice/email), drops unknown fields, type-checks.
- Output is typed `ValidatedData` with `content_hash`.

You then pass `validated.fields.amount`, etc, into UBL programs/chips.  
This ensures **data never becomes executable logic**, regardless of what strings contain.

### 2) Atomic Operations (JSON✯Atomic)
**Kernel responsibility:**
- Effects apply atomically.
- Ledger commits are crash-safe.
- EffectRecords are chain-hashed and can be signed.

This repo signs:
- `Proof.proof_hash` (optional)
- `EffectRecord.record_hash` (optional)

### 3) Shadow Validation
**Best expressed as:** Chips + Programs.

Pattern:
- Add a Shadow policy chip (boolean) that checks anomaly signals (limits, counterparties, approvals).
- Integrate into the *same* decision chip (extra gates) OR wrap in a “trusted program” that refuses on deny.

### 4) Trajectory-Based Identity
**Best expressed as:** Ledger + Chips.

Pattern:
- Maintain `agents.{id}.trust_score`, `agents.{id}.created_at`, `agents.{id}.capabilities`.
- Chips gate capabilities based on thresholds and age (use `age()` / `before()` / `after()`).

### 5) Circuit Breakers
**Best expressed as:** Chips + Ledger state (+ a few kernel built-ins).

This repo includes built-ins that make this workable:
- `add(a,b)`, `sub(a,b)` for thresholds
- `time_bucket(ts, unit)` for hour/day bucketing
- `now()` deterministic per execution
- optional: store counters in ledger under `breakers.{agent_id}`

### 6) Multi-Signature Operations
Two options:
1) **Program-level approvals** (portable): store approvals in ledger and require `length(approvals) >= threshold`.
2) **Cryptographic verification** (kernel primitive): use `verify_ed25519(pk_b64, msg, sig_b64)` inside a chip gate.

This repo includes `verify_ed25519` as a built-in function for the second model.

See `examples/trust/` for templates.

## Included Trust program pack

If you want a runnable starting point rather than templates, register:

- `stdlib/program_packs/trust_programs.json`

This pack contains:
- shadow registration + verification programs
- circuit breaker creation + update + evaluation programs
- a multisig-style transfer orchestration program
