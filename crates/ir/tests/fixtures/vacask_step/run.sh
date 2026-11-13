#!/usr/bin/env bash
cd "$(dirname "$0")" || exit 2
. ../checked.sh
echo "vacask $(readlink -f "$(which vacask)")"
for f in *.sim; do n=${f%.sim}
  rm -f ./*.raw
  if va "$f"; then echo "$n $(python3 steps.py tran1.raw)"; else echo "$n refused($WHY)"; fi
done
rm -f ./*.raw
exit $REFUSED
