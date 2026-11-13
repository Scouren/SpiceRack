#!/usr/bin/env bash
# F52 localised (R84a): the dio.sim diode (rs=1 is=1e-9 tt=1u cjo=50n bv=5 ibv=1u) on both simulators the shell puts
# on PATH: DC at forward bias (tight tolerances), AC at 1 kHz and 100 kHz (re, im of i(v1); diffusion and depletion
# charge), and the reverse-recovery transient's i(v1) at 0.4 us and 0.5 us under default and tightened tolerances and
# step caps down to 4 ps. A run that did not complete is refused (../../checked.sh).
cd "$(dirname "$0")" || exit 2
. ../../checked.sh
TMO=600
echo "ngspice $(readlink -f "$(which ngspice)")  vacask $(readlink -f "$(which vacask)")"
for f in dc_*.cir; do n=${f%.cir}
  if ng "$f"; then ngv=$(printf '%s\n' "$OUT" | grep -E "^i\(v1\)" | awk '{print $NF}'); else ngv="refused($WHY)"; fi
  rm -f ./*.raw
  if va "$n.sim"; then vav=$(python3 ../raw.py op1.raw i:v1); else vav="refused($WHY)"; fi
  echo "$n ngspice=$ngv vacask=$vav"
done
for f in ac_*.cir; do n=${f%.cir}
  if ng "$f"; then ngv=$(printf '%s\n' "$OUT" | grep -E "^(real|imag)" | awk '{print $NF}' | tr '\n' ' '); else ngv="refused($WHY)"; fi
  rm -f ./*.raw
  if va "$n.sim"; then vav=$(python3 rawc.py ac1.raw 'v1:flow(br)'); else vav="refused($WHY)"; fi
  echo "$n ngspice=[$ngv] vacask=[$vav]"
done
for f in tr_*.cir; do n=${f%.cir}
  if ng "$f"; then ngv=$(printf '%s\n' "$OUT" | grep -E "^i(40|50) " | awk '{print $1"="$3}' | tr '\n' ' '); else ngv="refused($WHY)"; fi
  rm -f ./*.raw
  if va "$n.sim"; then vav=$(python3 ../tranread.py tran1.raw 'v1:flow(br)' 0.4u 0.5u | tr '\n' ' '); else vav="refused($WHY)"; fi
  echo "$n ngspice[$ngv] vacask[$vav]"
done
rm -f ./*.raw
exit $REFUSED
