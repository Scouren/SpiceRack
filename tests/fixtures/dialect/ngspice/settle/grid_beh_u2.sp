grid_beh_u2
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
va9 a9 0 dc 0.25
va10 a10 0 dc 0.5
va11 a11 0 dc 0.75
va12 a12 0 dc 1
va13 a13 0 dc 1.5
va14 a14 0 dc 2.5
va15 a15 0 dc 3
va16 a16 0 dc 1000.0
.nodeset v(a0)=-1000.0 v(a1)=-3 v(a2)=-2.5 v(a3)=-1.5 v(a4)=-1 v(a5)=-0.5 v(a6)=-1e-07 v(a7)=0 v(a8)=1e-07 v(a9)=0.25 v(a10)=0.5 v(a11)=0.75 v(a12)=1 v(a13)=1.5 v(a14)=2.5 v(a15)=3 v(a16)=1000.0
bd510 d510 0 v = u2(v(a1))
bd511 d511 0 v = u2(v(a2))
bd512 d512 0 v = u2(v(a3))
bd513 d513 0 v = u2(v(a4))
bd514 d514 0 v = u2(v(a5))
bd515 d515 0 v = u2(v(a6))
bd516 d516 0 v = u2(v(a7))
bd517 d517 0 v = u2(v(a8))
bd518 d518 0 v = u2(v(a10))
bd519 d519 0 v = u2(v(a12))
bd520 d520 0 v = u2(v(a13))
bd521 d521 0 v = u2(v(a14))
bd522 d522 0 v = u2(v(a15))
bd523 d523 0 v = u2(v(a0))
bd524 d524 0 v = u2(v(a16))
bd525 d525 0 v = u2(v(a9))
bd526 d526 0 v = u2(v(a11))
.op
.end
