level_invalid
* oracle: inpfindl.c (a level above 99 or below 0 runs level 1: run level=100 prints Level must be < 99 (Setting Level to 1)), inpdomod.c (MOS 7 and 15 not in this binary, BJT 3, JFET 3, MES 7: no device), inpcompat.c:282 (ako: only in PSPICE mode; run: Unknown model type ako:q1 - ignored), runs: level=7, 15 could not find a valid modelname
.model a nmos level=100 vto=0.5
.model b nmos level=7
.model c pmos level=15
.model d nmos level=-2
.model e npn level=3
.model f njf level=3
.model g nmf level=7
.model h ako:g nmf
