#!/usr/bin/env bash
# R102's default temperature and tnom on ngspice-45 and VACASK: 1 mA into a diode (is=1e-14)
# with no temperature statement, with temp=27, tnom=27, and temp=25 (the control): the
# statement-free run equals temp=27 and tnom=27, and differs from temp=25.
cd "$(dirname "$0")" || exit 2
. ../checked.sh
R=../families/raw.py
echo "ngspice $(readlink -f "$(which ngspice)")  vacask $(readlink -f "$(which vacask)")"
for c in none temp27 tnom27 temp25; do
  if ng ng_$c.cir; then echo "ngspice $c $(printf '%s\n' "$OUT" | grep -E '^v\(a\) =')"; else echo "ngspice $c refused($WHY)"; fi
  rm -f ./*.raw
  if va va_$c.sim; then echo "vacask  $c v(a) = $(python3 $R op1.raw v:a)"; else echo "vacask $c refused($WHY)"; fi
done
rm -f ./*.raw
exit $REFUSED
