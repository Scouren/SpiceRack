#!/usr/bin/env bash
# gf180_run.sh: the gf180 load decks (load.py gf180_n1, gf180_n10, gf180_nf). Makes kit/ from
# $PDK_ROOT/gf180mcuD (the kit `nix develop .#testing` puts there): its ngspice files as they are,
# and VACASK's models made from them by VACASK's own converter (python -m vacask.gf180tovc), then
# runs load_run.sh on each family (checked runs).
cd "$(dirname "$0")" || exit 2
rm -rf kit && mkdir -p kit/gf180mcuD/libs.tech || exit 2
cp -r "$PDK_ROOT/gf180mcuD/libs.tech/ngspice" kit/gf180mcuD/libs.tech/ && chmod -R u+w kit || exit 2
pp=$(vacask -dp 2>&1 | grep -i "python path addition" | sed 's/.*: *//')
(PYTHONPATH="$pp" PDK_ROOT="$PWD/kit" PDK=gf180mcuD python3 -m vacask.gf180tovc > kit/convert.log 2>&1) || exit 2
echo "kit: $PDK_ROOT/gf180mcuD, converted by $pp/vacask/gf180tovc.py"
rc=0
for f in gf180_n1 gf180_n10 gf180_nf; do echo "### $f"; ./load_run.sh "$f" || rc=1; done
exit $rc
