#!/usr/bin/env bash
# each deck on both simulators the shell puts on PATH; i(v1) read, or the simulator's words
cd "$(dirname "$0")" || exit 2
R=../families/raw.py
echo "ngspice $(readlink -f "$(which ngspice)")  vacask $(readlink -f "$(which vacask)")"
for f in ng_*.cir; do
  out=$(timeout 30 ngspice -b "$f" 2>&1); rc=$?
  echo "## $f rc=$rc"; printf '%s\n' "$out" | grep -iE "i\(v1\) =|error|warning|unknown|syntax|cannot|undefined" | head -6
done
for f in va_*.sim; do
  rm -f ./*.raw; out=$(timeout 30 vacask "$f" 2>&1); rc=$?
  echo "## $f rc=$rc"; printf '%s\n' "$out" | sed -n '/Simulating:/,$p' | grep -v "^Simulating\|Running\|Elapsed" | head -6
  [ -f op1.raw ] && echo "i(v1)=$(python3 $R op1.raw i:v1)"
  printf '%s\n' "$out" | sed -n '1,/Simulating:/p' | grep -iE "error|not|cannot" | head -3
done
rm -f ./*.raw
