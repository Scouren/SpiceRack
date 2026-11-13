grid_beh_acos
* settle grid: values in the raw file (ngspice -b -r), compared with the IR's reference evaluator by tests/dialect_ngspice_grid.rs
va0 a0 0 dc -0.99
va1 a1 0 dc -0.5
va2 a2 0 dc -1e-07
va3 a3 0 dc 0
va4 a4 0 dc 1e-07
va5 a5 0 dc 0.5
va6 a6 0 dc 0.99
.nodeset v(a0)=-0.99 v(a1)=-0.5 v(a2)=-1e-07 v(a3)=0 v(a4)=1e-07 v(a5)=0.5 v(a6)=0.99
bd118 d118 0 v = acos(v(a0))
bd119 d119 0 v = acos(v(a1))
bd120 d120 0 v = acos(v(a2))
bd121 d121 0 v = acos(v(a3))
bd122 d122 0 v = acos(v(a4))
bd123 d123 0 v = acos(v(a5))
bd124 d124 0 v = acos(v(a6))
.op
.end
