grid_beh_atan
* settle grid: values in the raw file (ngspice -b -r), compared with the IR's reference evaluator by tests/dialect_ngspice_grid.rs
va0 a0 0 dc -1000.0
va1 a1 0 dc -3
va2 a2 0 dc -2.5
va3 a3 0 dc -1.5
va4 a4 0 dc -1
va5 a5 0 dc -0.5
va6 a6 0 dc -1e-07
va7 a7 0 dc 0
va8 a8 0 dc 1e-07
va9 a9 0 dc 0.5
va10 a10 0 dc 1
va11 a11 0 dc 1.5
va12 a12 0 dc 2.5
va13 a13 0 dc 3
va14 a14 0 dc 1000.0
.nodeset v(a0)=-1000.0 v(a1)=-3 v(a2)=-2.5 v(a3)=-1.5 v(a4)=-1 v(a5)=-0.5 v(a6)=-1e-07 v(a7)=0 v(a8)=1e-07 v(a9)=0.5 v(a10)=1 v(a11)=1.5 v(a12)=2.5 v(a13)=3 v(a14)=1000.0
bd126 d126 0 v = atan(v(a1))
bd127 d127 0 v = atan(v(a2))
bd128 d128 0 v = atan(v(a3))
bd129 d129 0 v = atan(v(a4))
bd130 d130 0 v = atan(v(a5))
bd131 d131 0 v = atan(v(a6))
bd132 d132 0 v = atan(v(a7))
bd133 d133 0 v = atan(v(a8))
bd134 d134 0 v = atan(v(a9))
bd135 d135 0 v = atan(v(a10))
bd136 d136 0 v = atan(v(a11))
bd137 d137 0 v = atan(v(a12))
bd138 d138 0 v = atan(v(a13))
bd139 d139 0 v = atan(v(a0))
bd140 d140 0 v = atan(v(a14))
.op
.end
