grid_beh_sin
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
bd63 d63 0 v = sin(v(a1))
bd64 d64 0 v = sin(v(a2))
bd65 d65 0 v = sin(v(a3))
bd66 d66 0 v = sin(v(a4))
bd67 d67 0 v = sin(v(a5))
bd68 d68 0 v = sin(v(a6))
bd69 d69 0 v = sin(v(a7))
bd70 d70 0 v = sin(v(a8))
bd71 d71 0 v = sin(v(a9))
bd72 d72 0 v = sin(v(a10))
bd73 d73 0 v = sin(v(a11))
bd74 d74 0 v = sin(v(a12))
bd75 d75 0 v = sin(v(a13))
bd76 d76 0 v = sin(v(a0))
bd77 d77 0 v = sin(v(a14))
.op
.end
