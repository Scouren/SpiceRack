#!/usr/bin/env bash
# load_run.sh FAMILY: every load deck of FAMILY (load.py) on both simulators the shell puts on PATH:
# deck, ngspice-45's printed i(vd) or |i(vd)|, VACASK's, and |a-b|/max(|a|,|b|); then fd.py's oracle
# lines where the family has its derivative probes. A run that did not complete gives
# refused(<why>) in place of its reading (../checked.sh, R91b) and the script exits 1.
cd "$(dirname "$0")/$1" || exit 2
. ../../checked.sh
echo "ngspice $(readlink -f "$(which ngspice)")  vacask $(readlink -f "$(which vacask)")"
rows=$(for f in *.cir; do n=${f%.cir}
  if ng "$f"; then
    ngv=$(printf '%s\n' "$OUT" | grep -E "^(i\(vd\)|mag\(i\(vd\)\))[^=]* = " | awk '{print $NF}' | tail -1)
  else ngv="refused($WHY)"; fi
  rm -f ./*.raw
  if va "$n.sim"; then r=$(ls ./*.raw 2>/dev/null | head -1); vav=$( [ -n "$r" ] && python3 ../raw.py "$r" i:vd )
  else vav="refused($WHY)"; fi
  rel=$(python3 -c "import sys; a,b=abs(float(sys.argv[1])),abs(float(sys.argv[2])); print('%.2e' % (abs(a-b)/max(a,b,1e-300)))" "$ngv" "$vav" 2>/dev/null)
  printf '%-22s ngspice=%-22s vacask=%-24s rel=%s\n' "$n" "$ngv" "$vav" "$rel"
done; exit $REFUSED); refused=$?
rm -f ./*.raw
printf '%s\n' "$rows"
printf '%s\n' "$rows" | python3 ../fd.py
exit $refused
