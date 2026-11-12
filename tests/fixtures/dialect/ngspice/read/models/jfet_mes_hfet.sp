jfet_mes_hfet
* oracle: inpdomod.c njf/pjf (0-1 JFET, 2 JFET2), nmf/pmf/nhfet/phfet (0-1 MES, 2-4 MESA with its level a parameter, mesa.c:108, 5 HFET1, 6 HFET2); the type word is the device's polarity flag (jfet.c, mes.c, hfet2.c)
.model j1 njf beta=1e-4 vto=-2
.model j2 pjf level=2 beta=1e-4
.model mf pmf vto=-2
.model h5 nhfet level=5 vto=0.1
.model h6 phfet level=6
.model ma pmf level=3 vto=-1
