grid_beh_floor
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
bd465 d465 0 v = floor(v(a1))
bd466 d466 0 v = floor(v(a2))
bd467 d467 0 v = floor(v(a3))
bd468 d468 0 v = floor(v(a4))
bd469 d469 0 v = floor(v(a5))
bd470 d470 0 v = floor(v(a6))
bd471 d471 0 v = floor(v(a7))
bd472 d472 0 v = floor(v(a8))
bd473 d473 0 v = floor(v(a9))
bd474 d474 0 v = floor(v(a10))
bd475 d475 0 v = floor(v(a11))
bd476 d476 0 v = floor(v(a12))
bd477 d477 0 v = floor(v(a13))
bd478 d478 0 v = floor(v(a0))
bd479 d479 0 v = floor(v(a14))
.op
.end
