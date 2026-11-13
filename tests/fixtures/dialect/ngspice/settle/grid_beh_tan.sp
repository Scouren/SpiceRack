grid_beh_tan
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
bd93 d93 0 v = tan(v(a1))
bd94 d94 0 v = tan(v(a2))
bd95 d95 0 v = tan(v(a3))
bd96 d96 0 v = tan(v(a4))
bd97 d97 0 v = tan(v(a5))
bd98 d98 0 v = tan(v(a6))
bd99 d99 0 v = tan(v(a7))
bd100 d100 0 v = tan(v(a8))
bd101 d101 0 v = tan(v(a9))
bd102 d102 0 v = tan(v(a10))
bd103 d103 0 v = tan(v(a11))
bd104 d104 0 v = tan(v(a12))
bd105 d105 0 v = tan(v(a13))
bd106 d106 0 v = tan(v(a0))
bd107 d107 0 v = tan(v(a14))
.op
.end
