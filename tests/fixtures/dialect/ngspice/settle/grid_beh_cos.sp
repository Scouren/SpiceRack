grid_beh_cos
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
bd78 d78 0 v = cos(v(a1))
bd79 d79 0 v = cos(v(a2))
bd80 d80 0 v = cos(v(a3))
bd81 d81 0 v = cos(v(a4))
bd82 d82 0 v = cos(v(a5))
bd83 d83 0 v = cos(v(a6))
bd84 d84 0 v = cos(v(a7))
bd85 d85 0 v = cos(v(a8))
bd86 d86 0 v = cos(v(a9))
bd87 d87 0 v = cos(v(a10))
bd88 d88 0 v = cos(v(a11))
bd89 d89 0 v = cos(v(a12))
bd90 d90 0 v = cos(v(a13))
bd91 d91 0 v = cos(v(a0))
bd92 d92 0 v = cos(v(a14))
.op
.end
