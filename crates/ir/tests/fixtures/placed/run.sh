#!/usr/bin/env bash
# Placements on ngspice-45 (the lead's ruling on the census's placements, rewrite/IR-PRUNE.md
# group 6): each br_<card>_<sel> deck holds <card> in `.if (sel==1)`; a card the branch governs
# reads differently at sel=0 and sel=1, one ngspice keeps as if unconditional reads the same.
cd "$(dirname "$0")" || exit 2
. ../checked.sh
echo "ngspice $(readlink -f "$(which ngspice)")"
for f in br_*_0.cir br_*_1.cir lib_sections.cir sub_save.cir; do
  if ng "$f"; then
    echo "## $f"; printf '%s\n' "$OUT" | grep -E '^v\(b\) =|^i\(vg\) =|^vb |^va |^0\s|^[0-9]\s|CONTROL_IN|^\s+[a-z0-9.#()]+\s+: voltage|Initial Transient Solution|Operating Point|^v\(c\)' | grep -v '^\s*$' | head -6
  else echo "## $f refused($WHY)"; fi
done
exit $REFUSED
