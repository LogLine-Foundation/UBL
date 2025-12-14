# ubl_core — UBL 2.0 Kernel (Spec-Grade) + Trust Architecture (Publishable)

This repo is a **publishable** Rust reference implementation of the UBL 2.0 Kernel plus core Trust Architecture primitives.

## Why this exists
UBL treats **trust as architecture**: data cannot become instructions; decisions are content-addressed; every state change is provable and tamper-evident.

## What you get
### Kernel / Core
- Chips (pure decisions), Programs (orchestration), Proofs (verifiable decisions), Ledger (append-only history)
- **JCS (RFC8785)** canonicalization for all hashes
- **Atomic persistence** (tmp → fsync → rename → dir sync)
- **EffectRecord chaining** (`previous_record_hash` + `record_hash`)
- **Optional Ed25519 proof signatures** + `/verify`
- **Optional Ed25519 effect record signatures** (`record_signature`)
- **Registry introspection** endpoints

### Trust Architecture (runtime-level)
- **Isolation Barrier** endpoint: `POST /barrier/process`
  - Deterministic schema enforcement + type checking
  - Unknown fields dropped (anti prompt-injection)
  - Produces `{content_hash, fields}` (typed, non-executable)

See `docs/TRUST_INTEGRATION.md` for mapping.

## Run
```bash
cp .env.example .env
cargo build --release
./target/release/ubl_core
```

## Auth (optional)
Set `UBL_API_KEY` then send header:
```
x-ubl-key: <value>
```

## Endpoints
- `GET /health`
- `POST /register`  (chip/program)
- `POST /execute`
- `POST /verify`
- `GET /registry/chips`
- `GET /registry/programs`
- `POST /barrier/process`

## Ed25519 keys (optional)
Set base64 raw bytes:
- `UBL_ED25519_PRIVATE_KEY_B64` (32 bytes)
- `UBL_ED25519_PUBLIC_KEY_B64` (optional; derived from private)

## Trust examples
See `examples/trust/` for chips/program templates (circuit breaker, shadow, trajectory gating).

## Standard Library program packs

We include your existing **program packs** in:

- `stdlib/program_packs/programs_part1.json`
- `stdlib/program_packs/programs_part2.json`
- `stdlib/program_packs/trust_programs.json`

Load them into a running kernel:

```bash
python3 scripts/register_pack.py stdlib/program_packs/programs_part1.json
python3 scripts/register_pack.py stdlib/program_packs/programs_part2.json
python3 scripts/register_pack.py stdlib/program_packs/trust_programs.json
```

Details: `docs/STDLIB_PROGRAMS.md`.
