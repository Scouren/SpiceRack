#!/usr/bin/env bash
# R82's decks on both simulators the shell puts on PATH: v(b) before and after the alter; a run
# that did not complete gives refused(<why>) (../checked.sh, R91b)
cd "$(dirname "$0")" || exit 2
. ../checked.sh
echo "ngspice $(which ngspice)  vacask $(which vacask)"
for n in section if; do
  if ng "$n.cir"; then ngv=$(printf '%s\n' "$OUT" | grep -E '^v\(b\) = ' | awk '{printf "%s ", $3}'); else ngv="refused($WHY)"; fi
  rm -f ./*.raw
  if va "$n.sim"; then vav="op1 $(python3 ../families/raw.py op1.raw v:b) op2 $(python3 ../families/raw.py op2.raw v:b)"
  else vav="refused($WHY)"; fi
  echo "$n ngspice=$ngv vacask=$vav"
done
rm -f ./*.raw
exit $REFUSED
