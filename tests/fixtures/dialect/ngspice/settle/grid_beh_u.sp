grid_beh_u
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
bd495 d495 0 v = u(v(a1))
bd496 d496 0 v = u(v(a2))
bd497 d497 0 v = u(v(a3))
bd498 d498 0 v = u(v(a4))
bd499 d499 0 v = u(v(a5))
bd500 d500 0 v = u(v(a6))
bd501 d501 0 v = u(v(a7))
bd502 d502 0 v = u(v(a8))
bd503 d503 0 v = u(v(a9))
bd504 d504 0 v = u(v(a10))
bd505 d505 0 v = u(v(a11))
bd506 d506 0 v = u(v(a12))
bd507 d507 0 v = u(v(a13))
bd508 d508 0 v = u(v(a0))
bd509 d509 0 v = u(v(a14))
.op
.end
