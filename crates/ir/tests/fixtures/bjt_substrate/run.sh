#!/usr/bin/env bash
cd "$(dirname "$0")" || exit 2
echo "ngspice $(which ngspice)  vacask $(which vacask)"
for f in t*.cir; do n=${f%.cir}
  ng=$(timeout 30 ngspice -b $f 2>&1 | grep -i "i(vs)" | awk '{print $NF}')
  rm -f op1.raw; err=$(timeout 30 vacask $n.sim 2>&1 | grep -iE "^error" | head -1)
  va=$(awk '/^Variables:/{v=1;next} /^Values:/{v=0;p=1;next} v && $2=="vs:flow(br)"{k=$1} p{vals[i++]=$NF} END{print vals[k]}' op1.raw 2>/dev/null)
  echo "$n ngspice=$ng vacask=$va $err"
done
