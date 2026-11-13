* ngspice twin
.lib a
.param pa=1
.endl a
.lib b
.param pb={zz*2}
.endl b
.lib c
.param pc={zz*3}
.endl c
.lib d
.lib sel.lib.sp c
.param pd=1
.endl d
