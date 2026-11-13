#!/usr/bin/env bash
# load_run.sh FAMILY: every load deck of FAMILY (load.py) on both simulators the shell puts on PATH:
# deck, ngspice-45's printed i(vd) or |i(vd)|, VACASK's, and |a-b|/max(|a|,|b|)
cd "$(dirname "$0")/$1" || exit 2
echo "ngspice $(readlink -f "$(which ngspice)")  vacask $(readlink -f "$(which vacask)")"
for f in *.cir; do n=${f%.cir}
  ng=$(timeout 60 ngspice -b "$f" 2>&1 | grep -E "^(i\(vd\)|mag\(i\(vd\)\))[^=]* = " | awk '{print $NF}' | tail -1)
  rm -f ./*.raw; err=$(timeout 60 vacask "$n.sim" 2>&1 | grep -iE "^error|not found" | head -1)
  r=$(ls ./*.raw 2>/dev/null | head -1); va=$( [ -n "$r" ] && python3 ../raw.py "$r" i:vd )
  rel=$(python3 -c "import sys; a,b=abs(float(sys.argv[1])),abs(float(sys.argv[2])); print('%.2e' % (abs(a-b)/max(a,b,1e-300)))" "$ng" "$va" 2>/dev/null)
  printf '%-16s ngspice=%-22s vacask=%-24s rel=%s %s\n' "$n" "$ng" "$va" "$rel" "$err"
done
rm -f ./*.raw
