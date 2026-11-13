#!/usr/bin/env bash
# the diode leads on both simulators the shell puts on PATH: deck, ngspice-45's printed value, VACASK's
cd "$(dirname "$0")" || exit 2
R=../raw.py
echo "ngspice $(which ngspice)  vacask $(which vacask)"
for f in *.cir; do n=${f%.cir}
  ng=$(timeout 60 ngspice -b $f 2>&1 | grep -E "^(i\(v1\)|mag\(i\(v1\)\)|v\(s,a\))[^=]* = " | awk '{print $NF}' | tail -1)
  case $n in l3_*) e="v:s-a" ;; *) e="i:v1" ;; esac
  rm -f *.raw; timeout 60 vacask $n.sim >/dev/null 2>&1
  r=$(ls *.raw 2>/dev/null | head -1); va=$( [ -n "$r" ] && python3 $R $r $e )
  echo "$n ngspice=$ng vacask=$va"
done
