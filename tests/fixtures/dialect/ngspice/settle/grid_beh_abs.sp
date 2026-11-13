grid_beh_abs
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
bd0 d0 0 v = abs(v(a1))
bd1 d1 0 v = abs(v(a2))
bd2 d2 0 v = abs(v(a3))
bd3 d3 0 v = abs(v(a4))
bd4 d4 0 v = abs(v(a5))
bd5 d5 0 v = abs(v(a6))
bd6 d6 0 v = abs(v(a7))
bd7 d7 0 v = abs(v(a8))
bd8 d8 0 v = abs(v(a9))
bd9 d9 0 v = abs(v(a10))
bd10 d10 0 v = abs(v(a11))
bd11 d11 0 v = abs(v(a12))
bd12 d12 0 v = abs(v(a13))
bd13 d13 0 v = abs(v(a0))
bd14 d14 0 v = abs(v(a14))
.op
.end
