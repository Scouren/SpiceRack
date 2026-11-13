#!/usr/bin/env bash
# every deck on both simulators the shell puts on PATH: i(vs), the substrate source's current;
# a run that did not complete gives refused(<why>) in place of its reading (../checked.sh, R91b)
cd "$(dirname "$0")" || exit 2
. ../checked.sh
echo "ngspice $(which ngspice)  vacask $(which vacask)"
for f in t*.cir; do n=${f%.cir}
  if ng "$f"; then ngv=$(printf '%s\n' "$OUT" | grep -i "i(vs)" | awk '{print $NF}'); else ngv="refused($WHY)"; fi
  rm -f op1.raw
  if va "$n.sim"; then
    vav=$(awk '/^Variables:/{v=1;next} /^Values:/{v=0;p=1;next} v && $2=="vs:flow(br)"{k=$1} p{vals[i++]=$NF} END{print vals[k]}' op1.raw 2>/dev/null)
  else vav="refused($WHY)"; fi
  echo "$n ngspice=$ngv vacask=$vav"
done
rm -f op1.raw
exit $REFUSED
