#!/usr/bin/env bash
# run.sh [deck.sp ...]: every HSPICE settle deck (default: all) on ESPice in HSPICE mode
# (`espice-hspice --tokenizer=hspice`, the flake's pinned ESPice 0.3.0; ESPice is not HSPICE), each
# in a fresh copy of this folder: exit code, ESPice's error and warning lines, its print table
# (`--format=print`) and its measures (`--measures=-`). The ngspice twins (`*.ng.cir`) run with
# `ngspice -b`. Run inside `nix develop .#testing`; the record is runs.out.
SET=$(cd "$(dirname "$0")" && pwd); W=$(mktemp -d); trap 'rm -rf "$W"' EXIT
echo "ESPice: $(espice-hspice -v 2>&1 | head -1)"
cd "$SET"; decks=${*:-$(ls *.sp)}
for d in $decks; do rm -rf "$W/d"; mkdir "$W/d"; cp -r "$SET"/. "$W/d"/; cd "$W/d"
  timeout 120 espice-hspice --tokenizer=hspice --format=print -r out.txt --measures=m.json "$d" > log 2>&1; rc=$?
  echo "== $d rc=$rc"
  grep -iE 'error|warn|unsupported|refus|ignor|unknown' log | sed 's/^ *//' | sort -u | head -4 | sed 's/^/  ! /'
  [ -f m.json ] && grep -E '"name"' m.json | sed -E 's/.*"name": "([^"]*)".*"value": ([^,]*),.*"error": ([^}]*)\}.*/  m \1 = \2 \3/' | head -6
  [ -f out.txt ] && head -14 out.txt | cut -c1-400 | sed 's/^/  | /'
  cd "$SET"
done
[ -n "$*" ] && exit 0
for t in *.ng.cir; do echo "== ngspice $t"; ngspice -b "$t" 2>&1 | grep -E '^[a-z(][^=]* = ' | sed 's/^/  | /'; done
