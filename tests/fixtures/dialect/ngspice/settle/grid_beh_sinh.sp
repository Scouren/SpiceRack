grid_beh_sinh
* settle grid: values in the raw file (ngspice -b -r), compared with the IR's reference evaluator by tests/dialect_ngspice_grid.rs
va0 a0 0 dc -30
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
va14 a14 0 dc 30
.nodeset v(a0)=-30 v(a1)=-3 v(a2)=-2.5 v(a3)=-1.5 v(a4)=-1 v(a5)=-0.5 v(a6)=-1e-07 v(a7)=0 v(a8)=1e-07 v(a9)=0.5 v(a10)=1 v(a11)=1.5 v(a12)=2.5 v(a13)=3 v(a14)=30
bd141 d141 0 v = sinh(v(a1))
bd142 d142 0 v = sinh(v(a2))
bd143 d143 0 v = sinh(v(a3))
bd144 d144 0 v = sinh(v(a4))
bd145 d145 0 v = sinh(v(a5))
bd146 d146 0 v = sinh(v(a6))
bd147 d147 0 v = sinh(v(a7))
bd148 d148 0 v = sinh(v(a8))
bd149 d149 0 v = sinh(v(a9))
bd150 d150 0 v = sinh(v(a10))
bd151 d151 0 v = sinh(v(a11))
bd152 d152 0 v = sinh(v(a12))
bd153 d153 0 v = sinh(v(a13))
bd154 d154 0 v = sinh(v(a0))
bd155 d155 0 v = sinh(v(a14))
.op
.end
