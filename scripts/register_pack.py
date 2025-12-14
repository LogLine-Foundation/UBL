#!/usr/bin/env python3
"""Register a chip/program pack JSON into a running ubl_core instance.

Pack format accepted:
- {"programs": [ ...Program... ]}
- {"chips": [ ...Chip... ]}
- {"programs": [...], "chips": [...]}

Usage:
  python3 scripts/register_pack.py stdlib/program_packs/programs_part1.json

Env:
  UBL_URL=http://localhost:8000
  UBL_API_KEY=...   (optional; sent as x-ubl-key)
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.request
import urllib.error


def post_json(url: str, payload: dict) -> dict:
    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(url, data=data, method="POST")
    req.add_header("Content-Type", "application/json")

    api_key = os.environ.get("UBL_API_KEY")
    if api_key:
        req.add_header("x-ubl-key", api_key)

    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            body = resp.read().decode("utf-8")
            return json.loads(body) if body else {}
    except urllib.error.HTTPError as e:
        body = e.read().decode("utf-8")
        raise RuntimeError(f"HTTP {e.code} {e.reason}: {body}")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("pack", help="Path to a JSON pack containing chips/programs")
    args = ap.parse_args()

    base = os.environ.get("UBL_URL", "http://localhost:8000").rstrip("/")
    reg_url = f"{base}/register"

    with open(args.pack, "r", encoding="utf-8") as f:
        pack = json.load(f)

    chips = pack.get("chips", [])
    programs = pack.get("programs", [])

    if not chips and not programs:
        print("Pack has no 'chips' or 'programs' arrays.", file=sys.stderr)
        return 2

    # Register chips first (programs may reference CHIP:<name>)
    for c in chips:
        res = post_json(reg_url, {"type": "chip", "data": c})
        print(f"chip  {c.get('name','?'):>24}  -> {res.get('hash','?')}")

    for p in programs:
        res = post_json(reg_url, {"type": "program", "data": p})
        print(f"prog  {p.get('name','?'):>24}  -> {res.get('hash','?')}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
