# Standard Library Program Packs

This repo ships **program packs** (JSON) that you can register into a running kernel.

The packs included here came from your existing library:
- `stdlib/program_packs/programs_part1.json`
- `stdlib/program_packs/programs_part2.json`
- `stdlib/program_packs/trust_programs.json`

## Important: `evaluate: "CHIP:<name>"`

These programs reference chips by **name** via the form:

- `evaluate: "CHIP:transfer"`

The kernel resolves this at execution time using the **chip name index** (`registry.chip_names`).

That means:
1. Register the chips **first** (with matching `chip.name`).
2. Then register the programs.

If you prefer pure content addressing end-to-end, you can also rewrite `evaluate` to the chip hash after registration.

## Registering a pack

Start the server:

```bash
cargo run --release
```

Then register:

```bash
python3 scripts/register_pack.py stdlib/program_packs/programs_part1.json
python3 scripts/register_pack.py stdlib/program_packs/programs_part2.json
python3 scripts/register_pack.py stdlib/program_packs/trust_programs.json
```

Optional auth:

```bash
export UBL_API_KEY="..."
python3 scripts/register_pack.py stdlib/program_packs/programs_part1.json
```

Optional non-default URL:

```bash
export UBL_URL="http://127.0.0.1:8000"
python3 scripts/register_pack.py stdlib/program_packs/trust_programs.json
```

## Template interpolation in Effects

These packs make heavy use of placeholders like:

- `{amount}` / `{entity_id}`
- `{loan.borrower}`
- `{now}`
- `{proof.failed_gates}`

For publication, the kernel now resolves templates **deterministically** at apply-time and stores **resolved effects** in the `EffectRecord`.

This makes the ledger replayable without needing to rehydrate the original program input.
