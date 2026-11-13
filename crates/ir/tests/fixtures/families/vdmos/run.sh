#!/usr/bin/env bash
# the vdmos leads on both simulators the shell puts on PATH: deck, ngspice-45's printed value, VACASK's
cd "$(dirname "$0")" || exit 2
echo "ngspice $(which ngspice)  vacask $(which vacask)"
for f in *.cir; do n=${f%.cir}
  ng=$(timeout 60 ngspice -b $f 2>&1 | grep -E "^(i\(vd\)|mag\(i\(vd\)\))[^=]* = " | awk '{print $NF}' | tail -1)
  rm -f *.raw; err=$(timeout 60 vacask $n.sim 2>&1 | grep -iE "^error|not found" | head -1)
  r=$(ls *.raw 2>/dev/null | head -1); va=$( [ -n "$r" ] && python3 ../raw.py $r i:vd )
  echo "$n ngspice=$ng vacask=$va $err"
done
