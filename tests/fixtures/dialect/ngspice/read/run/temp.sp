temp
* oracle: ngspice-45 inp.c:1159-1186 (.temp t and .temp=t, strtod, the last .temp wins); runs temp5.cir (85 then 60 runs 60), .temp {t} runs t
.temp 85
.temp=60
.temp {t}
