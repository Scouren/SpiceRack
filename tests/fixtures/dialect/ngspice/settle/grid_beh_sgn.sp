grid_beh_sgn
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
bd435 d435 0 v = sgn(v(a1))
bd436 d436 0 v = sgn(v(a2))
bd437 d437 0 v = sgn(v(a3))
bd438 d438 0 v = sgn(v(a4))
bd439 d439 0 v = sgn(v(a5))
bd440 d440 0 v = sgn(v(a6))
bd441 d441 0 v = sgn(v(a7))
bd442 d442 0 v = sgn(v(a8))
bd443 d443 0 v = sgn(v(a9))
bd444 d444 0 v = sgn(v(a10))
bd445 d445 0 v = sgn(v(a11))
bd446 d446 0 v = sgn(v(a12))
bd447 d447 0 v = sgn(v(a13))
bd448 d448 0 v = sgn(v(a0))
bd449 d449 0 v = sgn(v(a14))
.op
.end
