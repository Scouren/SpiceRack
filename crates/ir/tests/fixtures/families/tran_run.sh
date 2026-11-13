#!/usr/bin/env bash
# tran_run.sh [DIR]: every deck of tran/ or DIR (tran.py) on both simulators the shell puts on PATH: v(d) at
# each time, ngspice-45's (meas) against VACASK's (raw, interpolated), and |a-b|/3 V. A run that did not
# complete is refused with its words (../checked.sh, R91b) and the script exits 1.
cd "$(dirname "$0")/${1:-tran}" || exit 2
. ../../checked.sh
TMO=120
echo "ngspice $(readlink -f "$(which ngspice)")  vacask $(readlink -f "$(which vacask)")"
T="2n 3n 4n 6n 8n 9n 12n 20n 30n"
for f in *.cir; do n=${f%.cir}
  ngv=""; vav=""; why=""
  if ng "$f"; then ngv=$(printf '%s\n' "$OUT" | grep -E "^t[0-9]+n +=" | awk '{print $3}'); else why="ngspice refused($WHY) "; fi
  rm -f ./*.raw
  if va "$n.sim"; then r=$(ls ./*.raw 2>/dev/null | head -1); vav=$( [ -n "$r" ] && python3 ../tranread.py "$r" d $T | awk '{print $2}')
  else why="${why}vacask refused($WHY)"; fi
  echo "## $n $why"
  [ -n "$why" ] && continue
  paste <(printf '%s\n' $T) <(printf '%s\n' $ngv) <(printf '%s\n' $vav) | \
    awk '{d=$2-$3; if (d<0) d=-d; printf "  t=%-4s ngspice=%-18s vacask=%-22s |d|/3V=%.2e\n", $1, $2, $3, d/3}'
done
rm -f ./*.raw
exit $REFUSED
