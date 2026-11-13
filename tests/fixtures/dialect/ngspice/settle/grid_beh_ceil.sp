grid_beh_ceil
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
bd450 d450 0 v = ceil(v(a1))
bd451 d451 0 v = ceil(v(a2))
bd452 d452 0 v = ceil(v(a3))
bd453 d453 0 v = ceil(v(a4))
bd454 d454 0 v = ceil(v(a5))
bd455 d455 0 v = ceil(v(a6))
bd456 d456 0 v = ceil(v(a7))
bd457 d457 0 v = ceil(v(a8))
bd458 d458 0 v = ceil(v(a9))
bd459 d459 0 v = ceil(v(a10))
bd460 d460 0 v = ceil(v(a11))
bd461 d461 0 v = ceil(v(a12))
bd462 d462 0 v = ceil(v(a13))
bd463 d463 0 v = ceil(v(a0))
bd464 d464 0 v = ceil(v(a14))
.op
.end
