# UBL Core (UBL 2.0 Kernel) — Publication Edition

**UBL (Universal Business Language) 2.0** turns *natural-language intent* into **immutable, verifiable business execution**.

This repository is a **minimal, production-oriented kernel** that:
- evaluates **Chips** (pure decisions) deterministically,
- executes **Programs** (orchestration + Effects) atomically against a versioned ledger,
- emits **cryptographic Proofs** for every decision,
- and supports a **Trust Architecture** that treats all inputs (including LLM output) as hostile until validated.

> If you can replay it, you can audit it. If you can audit it, you can trust it.

---

## Abstract

Modern systems fail in two ways:
1) decisions are opaque (“why did this transfer happen?”), and
2) LLM-driven automation increases surface area (prompt injection, context poisoning, silent drift).

UBL 2.0 solves this by making **every decision a content-addressed artifact**:
- **Chip**: pure boolean policy evaluated over explicit context
- **Program**: binds context + applies ordered, atomic effects
- **Ledger**: append-only effect history with state versioning
- **Proof**: replayable evidence that a chip evaluated correctly
- **Kernel**: the smallest trusted code that runs everything else

UBL’s goal is not “smarter automation”. It’s **auditable automation**.

---

## What’s Included

### Kernel capabilities
- Deterministic expression evaluation (no hidden state)
- Canonical JSON (JCS / RFC 8785 style) hashing for content-addressable IDs
- SHA-256 hashing for chips, proofs, and ledger records
- Optional **Ed25519 signing + verification** for proofs and ledger records
- Atomic persistence (`tmp → fsync → rename → dir sync`) for crash safety
- Optimistic concurrency with `target_version` checks
- Chain-hashed ledger records (`previous_record_hash → record_hash`) for tamper evidence
- Structured logging (tracing)

### Trust Architecture features
- **Isolation Barrier** endpoint to validate/normalize “untrusted input” into a typed envelope
- Standard-library program packs (including `trust_programs.json`) to enforce
  - circuit breakers,
  - multisig approvals,
  - vendor allowlists,
  - transfer limits,
  - escalation workflows.

The Trust Architecture is grounded in the principle that “trust must be computable and replayable”, and is designed to resist prompt injection and untrusted orchestration.

---

## UBL 2.0 in 60 seconds

### 1) Chip (decision)
A **Chip** is a pure policy:
- input: `context`
- output: `ALLOW (1)` or `DENY (0)`

It contains **Gates** (named conditions) combined by a **Composition** strategy (ALL/ANY/MAJORITY/WEIGHTED).

### 2) Program (action)
A **Program**:
- receives inputs,
- builds context (`input | ledger | computed`),
- evaluates a chip,
- applies ordered **Effects** (atomic, all-or-nothing),
- writes an **EffectRecord** to the ledger.

### 3) Proof (trust)
A **Proof** includes:
- chip hash,
- context snapshot,
- per-gate results,
- final result,
- deterministic proof hash,
- optional signature.

Anyone can verify by re-running the chip against the snapshot.

---

## Repository Layout

```text
ubl_core/
├── Cargo.toml
├── README.md
├── src/
│   ├── main.rs           # Axum server & routes
│   ├── api.rs            # HTTP API: execute/register/verify + registry + barrier
│   ├── engine.rs         # Deterministic evaluation, JCS hashing, signatures
│   ├── ledger.rs         # Atomic persistence + versioned state + history chain
│   ├── types.rs          # Strict AST + request/response types
│   ├── trust_barrier.rs  # Isolation Barrier processor
│   └── ...
├── stdlib/
│   └── program_packs/
│       ├── trust_programs.json
│       ├── programs_part1.json
│       └── programs_part2.json
├── examples/
│   └── trust/
│       └── invoice_barrier.json
└── docs/
    ├── UBL-Trust-Architecture.md
    ├── TRUST_INTEGRATION.md
    └── PUBLISHING.md
```

---

## Quickstart

### Build & Run
```bash
cd ubl_core
cargo build --release
./target/release/ubl_core
```

By default the ledger is persisted to:
- `ubl_ledger.json` (in the working directory)

### Environment Variables
This build supports optional API auth and signing keys (recommended for publication deployments).

```bash
# API key for HTTP requests (optional but recommended)
export UBL_API_KEY="change-me"

# Optional signing keys (Ed25519). If present, the kernel signs proofs and ledger records.
export UBL_ED25519_SIGNING_KEY_B64="..."
export UBL_ED25519_VERIFYING_KEY_B64="..."
```

---

## HTTP API

Base URL: `http://localhost:8000`

### Health
```bash
curl http://localhost:8000/health
```

### Register a Chip or Program
```bash
curl -X POST http://localhost:8000/register \
  -H "content-type: application/json" \
  -H "x-ubl-key: $UBL_API_KEY" \
  -d '{
    "type": "chip",
    "data": { "name": "standard_transfer", "gates": [...], "composition": {"type":"ALL"} }
  }'
```

```bash
curl -X POST http://localhost:8000/register \
  -H "content-type: application/json" \
  -H "x-ubl-key: $UBL_API_KEY" \
  -d '{
    "type": "program",
    "data": { "name": "execute_transfer", "context": [...], "evaluate": "<chip_hash>", "on_allow": [...], "on_deny": [...] }
  }'
```

### Execute a Program
```bash
curl -X POST http://localhost:8000/execute \
  -H "content-type: application/json" \
  -H "x-ubl-key: $UBL_API_KEY" \
  -d '{
    "program": "execute_transfer",
    "inputs": { "from_id": "w1", "to_id": "w2", "amt": 100 },
    "target_version": 12
  }'
```

Response includes:
- `result` (ALLOWED / DENIED)
- `proof` (replayable decision evidence)
- `effect_record` (ledger block metadata)

### Verify a Proof
```bash
curl -X POST http://localhost:8000/verify \
  -H "content-type: application/json" \
  -H "x-ubl-key: $UBL_API_KEY" \
  -d '{ "proof": { ... }, "chip": { ... } }'
```

Returns `{ "valid": true|false }`.

### Registry Introspection
```bash
curl -H "x-ubl-key: $UBL_API_KEY" http://localhost:8000/registry/chips
curl -H "x-ubl-key: $UBL_API_KEY" http://localhost:8000/registry/programs
```

### Isolation Barrier (Trust Boundary)
```bash
curl -X POST http://localhost:8000/barrier/process \
  -H "content-type: application/json" \
  -H "x-ubl-key: $UBL_API_KEY" \
  -d '{
    "content_type": "invoice",
    "payload": { "vendor_id": "ACME", "amount": 149.99, "currency": "USD", "date": "2025-12-14" }
  }'
```

The barrier returns `validated_fields` (normalized) plus a deterministic `content_hash` that can be referenced from chips/programs.

---

## Standard Library Program Packs

This repo ships program packs intended as **governance scaffolding**.
They do **not** replace your business chips — they *wrap* them in trust controls.

Included packs:
- `stdlib/program_packs/trust_programs.json` (trust & governance workflows)
- `stdlib/program_packs/programs_part1.json`
- `stdlib/program_packs/programs_part2.json`

### What’s inside `trust_programs.json`
Examples of included programs:
- `TrustedTransfer`
- `MultisigTransfer`
- `VendorAllowlistAdd` / `VendorAllowlistRemove`
- `SetCircuitBreaker`
- `FreezeEntity` / `UnfreezeEntity`

These programs encode “how trust is applied” at runtime — approvals, limits, allowlists, and escalation — without expanding the kernel’s trusted surface.

> Philosophy: **make trust policy a Chip/Program concern**, not a kernel fork.

---

## Incorporating the Trust Architecture: Chip or Kernel?

**Answer: almost entirely as Chips + Programs.**

Kernel changes should be rare and limited to:
- deterministic hashing / canonicalization,
- deterministic evaluation semantics,
- signature primitives,
- atomic persistence & concurrency.

Everything else — including approvals, rate limits, circuit breakers, and “LLM output validation” — should be expressed as:
- **Barrier → ValidatedData**
- **Chips → decision policies**
- **Programs → orchestration and governance effects**

This mirrors the Trust Architecture’s framing: trust is a *replayable computation* rather than a “belief”, and the boundary between untrusted text and trusted execution must be explicit.

---

## Threat Model (publication-ready summary)

**Trusted**
- The kernel implementation (this repo)
- Hash + signature algorithms (SHA-256, Ed25519)
- Bootstrap assumptions (how initial chips/programs are installed)

**Untrusted**
- Program/chip payloads submitted by users
- External inputs (HTTP payloads, LLM outputs, vendor data)
- Any remote system referenced by orchestration

**Mitigations**
- Isolation Barrier: typed normalization + content hashing for untrusted payloads
- Content-addressed chips: policy is immutable once referenced by hash
- Replayable proofs: every decision is independently verifiable
- Chain-hashed history: ledger tampering is detectable
- Optional signatures: origin authentication for proofs and records
- Version checks: prevent concurrent “double spend” writes

---

## Publication Checklist

For a clean release:
- [ ] Configure `UBL_API_KEY` (or front with your gateway)
- [ ] Set Ed25519 keys if you want signed proofs/records
- [ ] Add a CI build (optional)
- [ ] Run smoke tests from `docs/PUBLISHING.md`
- [ ] Publish tags/releases, and include this README + docs

---

## License

Choose a license before publishing (MIT / Apache-2.0 recommended).
Add `LICENSE` and update the Cargo package metadata accordingly.

---

## Status

This is a **publication-oriented kernel**: small, auditable, and intentionally minimal.

If you want “enterprise features” (multi-tenancy, quotas, policy distribution, replication/consensus), keep them **outside** the kernel — as chips, programs, or sidecar services — so the trusted base stays small.

