#!/usr/bin/env bash
# the vdmos leads on both simulators the shell puts on PATH: deck, ngspice-45's printed value, VACASK's;
# a run that did not complete gives refused(<why>) in place of its reading (../../checked.sh, R91b)
cd "$(dirname "$0")" || exit 2
. ../../checked.sh
echo "ngspice $(which ngspice)  vacask $(which vacask)"
for f in *.cir; do n=${f%.cir}
  if ng "$f"; then
    ngv=$(printf '%s\n' "$OUT" | grep -E "^(i\(vd\)|mag\(i\(vd\)\))[^=]* = " | awk '{print $NF}' | tail -1)
  else ngv="refused($WHY)"; fi
  rm -f ./*.raw
  if va "$n.sim"; then r=$(ls ./*.raw 2>/dev/null | head -1); vav=$( [ -n "$r" ] && python3 ../raw.py "$r" i:vd )
  else vav="refused($WHY)"; fi
  echo "$n ngspice=$ngv vacask=$vav"
done
rm -f ./*.raw
exit $REFUSED
