#!/usr/bin/env bash
# R103: the PSpice switch on ngspice-45, two ways: the XSPICE pswitch log=TRUE written by hand
# (plain.cir) and a PSpice VSWITCH under ngbehavior=ltpsa (mode/, its .spiceinit); R = -1/i(vo)
# over the control sweep, side by side
cd "$(dirname "$0")" || exit 2
. ../checked.sh
echo "ngspice $(readlink -f "$(which ngspice)")"
plain=""; mode=""
if ng plain.cir; then plain=$(printf '%s\n' "$OUT" | grep -E "^[0-9]+\s"); else echo "plain refused($WHY)"; fi
cd mode || exit 2
if ng vswitch.cir; then mode=$(printf '%s\n' "$OUT" | grep -E "^[0-9]+\s"); else echo "mode refused($WHY)"; fi
cd ..
paste <(printf '%s\n' "$plain") <(printf '%s\n' "$mode")
exit $REFUSED
