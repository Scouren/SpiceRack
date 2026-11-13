#!/usr/bin/env bash
# gf180's typical nfet_03v3 bin card (../cards/gf180_nfet_03v3_4.params) and the BSIM4 defaults at
# nom_sat (w=1u l=0.28u, vgs 1.5, vds 3.3, vbs -0.5) under each version on each simulator
cd "$(dirname "$0")" || exit 2
python3 ver.py ../cards/gf180_nfet_03v3_4.params ../raw.py
rc=$?; rm -f v.cir v.sim ./*.raw; exit $rc
