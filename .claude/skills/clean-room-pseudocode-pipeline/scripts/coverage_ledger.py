#!/usr/bin/env python3
"""
Coverage ledger for the clean-room pseudocode pipeline.

Builds and maintains a checklist of every architectural unit (source file) and
every function/method inside it, so the Describe -> Implement -> Verify pipeline
can guarantee that *every architecture, every function* is covered and nothing is
silently dropped.

Function detection is heuristic and language-aware by file extension. It is meant
to seed a reviewable inventory, not to be a perfect parser -- review the ledger
with the user and prune/add entries before relying on it.

Status lifecycle per function: pending -> described -> implemented -> verified

Usage:
  python coverage_ledger.py init   --root <src-dir> --out ledger.json
  python coverage_ledger.py status --ledger ledger.json
  python coverage_ledger.py set    --ledger ledger.json --unit <file> --function <name> --status described
"""

import argparse
import json
import os
import re
import sys

STATUSES = ["pending", "described", "implemented", "verified"]

# Heuristic function/method patterns by language. Each pattern's group(1) is the name.
LANG_PATTERNS = {
    ".py":   [re.compile(r"^\s*def\s+([A-Za-z_]\w*)\s*\(")],
    ".pyi":  [re.compile(r"^\s*def\s+([A-Za-z_]\w*)\s*\(")],
    ".js":   [re.compile(r"^\s*(?:async\s+)?function\s+([A-Za-z_$]\w*)\s*\("),
              re.compile(r"^\s*(?:export\s+)?(?:const|let|var)\s+([A-Za-z_$]\w*)\s*=\s*(?:async\s*)?\(")],
    ".jsx":  None,  # filled below to reuse .js
    ".ts":   None,
    ".tsx":  None,
    ".go":   [re.compile(r"^\s*func\s+(?:\([^)]*\)\s*)?([A-Za-z_]\w*)\s*\(")],
    ".rs":   [re.compile(r"^\s*(?:pub\s+)?(?:async\s+)?fn\s+([A-Za-z_]\w*)\s*[\(<]")],
    ".java": [re.compile(r"^\s*(?:public|private|protected|static|final|\s)+[\w<>\[\],.?]+\s+([A-Za-z_]\w*)\s*\([^;]*\)\s*\{")],
    ".c":    [re.compile(r"^[A-Za-z_][\w\s\*<>:,&]*?\b([A-Za-z_]\w*)\s*\([^;]*\)\s*\{")],
    ".h":    None,
    ".cpp":  None,
    ".cc":   None,
    ".cxx":  None,
    ".hpp":  None,
    ".cu":   None,   # CUDA -> reuse C/C++
    ".cuh":  None,
    ".hip":  None,   # HIP -> reuse C/C++
}
# Reuse rules
_JS = LANG_PATTERNS[".js"]
for ext in (".jsx", ".ts", ".tsx"):
    LANG_PATTERNS[ext] = _JS
_C = LANG_PATTERNS[".c"]
for ext in (".h", ".cpp", ".cc", ".cxx", ".hpp", ".cu", ".cuh", ".hip"):
    LANG_PATTERNS[ext] = _C

SKIP_DIRS = {".git", "node_modules", "__pycache__", ".venv", "venv", "dist", "build", ".idea", ".vscode"}


def detect_functions(path):
    ext = os.path.splitext(path)[1].lower()
    patterns = LANG_PATTERNS.get(ext)
    if not patterns:
        return None  # unsupported extension -> not a code unit we scan
    names = []
    seen = set()
    try:
        with open(path, "r", encoding="utf-8", errors="ignore") as f:
            for line in f:
                for pat in patterns:
                    m = pat.match(line)
                    if m:
                        name = m.group(1)
                        # filter obvious keywords that C-style regex can catch
                        if name in {"if", "for", "while", "switch", "return", "sizeof"}:
                            continue
                        if name not in seen:
                            seen.add(name)
                            names.append(name)
    except OSError:
        return None
    return names


def cmd_init(args):
    root = os.path.abspath(args.root)
    if not os.path.isdir(root):
        print(f"error: not a directory: {root}", file=sys.stderr)
        return 1
    units = []
    total_funcs = 0
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
        for fn in sorted(filenames):
            full = os.path.join(dirpath, fn)
            funcs = detect_functions(full)
            if funcs is None:
                continue
            rel = os.path.relpath(full, root)
            entry = {
                "unit": rel,
                "functions": [{"name": n, "status": "pending"} for n in funcs],
            }
            units.append(entry)
            total_funcs += len(funcs)
    ledger = {"root": root, "target_language": args.language or "TBD", "units": units}
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(ledger, f, indent=2)
    print(f"Wrote {args.out}: {len(units)} units, {total_funcs} functions (all 'pending').")
    print("Review with the user and prune/add before Phase 1. Empty-function units may need manual entries.")
    return 0


def load(ledger_path):
    with open(ledger_path, "r", encoding="utf-8") as f:
        return json.load(f)


def cmd_status(args):
    ledger = load(args.ledger)
    counts = {s: 0 for s in STATUSES}
    not_verified = []
    for unit in ledger["units"]:
        for fn in unit["functions"]:
            counts[fn.get("status", "pending")] = counts.get(fn.get("status", "pending"), 0) + 1
            if fn.get("status") != "verified":
                not_verified.append((unit["unit"], fn["name"], fn.get("status", "pending")))
    total = sum(counts.values())
    print(f"Target language: {ledger.get('target_language', 'TBD')}")
    print(f"Total functions: {total}")
    for s in STATUSES:
        print(f"  {s:12} {counts.get(s, 0)}")
    if not_verified:
        print(f"\nNot yet verified ({len(not_verified)}):")
        for unit, name, st in not_verified:
            print(f"  [{st}] {unit} :: {name}")
    else:
        print("\nAll functions verified.")
    return 0


def cmd_set(args):
    if args.status not in STATUSES:
        print(f"error: status must be one of {STATUSES}", file=sys.stderr)
        return 1
    ledger = load(args.ledger)
    found = False
    for unit in ledger["units"]:
        if unit["unit"] != args.unit:
            continue
        for fn in unit["functions"]:
            if fn["name"] == args.function:
                fn["status"] = args.status
                found = True
    if not found:
        print(f"error: {args.unit} :: {args.function} not found in ledger", file=sys.stderr)
        return 1
    with open(args.ledger, "w", encoding="utf-8") as f:
        json.dump(ledger, f, indent=2)
    print(f"Set {args.unit} :: {args.function} -> {args.status}")
    return 0


def main():
    p = argparse.ArgumentParser(description="Coverage ledger for the clean-room pseudocode pipeline.")
    sub = p.add_subparsers(dest="cmd", required=True)

    pi = sub.add_parser("init", help="Scan a source tree and build the ledger.")
    pi.add_argument("--root", required=True)
    pi.add_argument("--out", default="ledger.json")
    pi.add_argument("--language", help="Target language for the reimplementation.")
    pi.set_defaults(func=cmd_init)

    ps = sub.add_parser("status", help="Show coverage counts and unverified functions.")
    ps.add_argument("--ledger", default="ledger.json")
    ps.set_defaults(func=cmd_status)

    pe = sub.add_parser("set", help="Update one function's status.")
    pe.add_argument("--ledger", default="ledger.json")
    pe.add_argument("--unit", required=True)
    pe.add_argument("--function", required=True)
    pe.add_argument("--status", required=True)
    pe.set_defaults(func=cmd_set)

    args = p.parse_args()
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
