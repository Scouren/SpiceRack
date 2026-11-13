grid_beh_uramp
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
bd480 d480 0 v = uramp(v(a1))
bd481 d481 0 v = uramp(v(a2))
bd482 d482 0 v = uramp(v(a3))
bd483 d483 0 v = uramp(v(a4))
bd484 d484 0 v = uramp(v(a5))
bd485 d485 0 v = uramp(v(a6))
bd486 d486 0 v = uramp(v(a7))
bd487 d487 0 v = uramp(v(a8))
bd488 d488 0 v = uramp(v(a9))
bd489 d489 0 v = uramp(v(a10))
bd490 d490 0 v = uramp(v(a11))
bd491 d491 0 v = uramp(v(a12))
bd492 d492 0 v = uramp(v(a13))
bd493 d493 0 v = uramp(v(a0))
bd494 d494 0 v = uramp(v(a14))
.op
.end
