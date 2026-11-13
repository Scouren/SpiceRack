grid_beh_sqrt
* settle grid: values in the raw file (ngspice -b -r), compared with the IR's reference evaluator by tests/dialect_ngspice_grid.rs
va0 a0 0 dc 1e-07
va1 a1 0 dc 0.5
va2 a2 0 dc 1
va3 a3 0 dc 1.5
va4 a4 0 dc 2.5
va5 a5 0 dc 3
va6 a6 0 dc 1000.0
.nodeset v(a0)=1e-07 v(a1)=0.5 v(a2)=1 v(a3)=1.5 v(a4)=2.5 v(a5)=3 v(a6)=1000.0
bd16 d16 0 v = sqrt(v(a0))
bd17 d17 0 v = sqrt(v(a1))
bd18 d18 0 v = sqrt(v(a2))
bd19 d19 0 v = sqrt(v(a3))
bd20 d20 0 v = sqrt(v(a4))
bd21 d21 0 v = sqrt(v(a5))
bd22 d22 0 v = sqrt(v(a6))
.op
.end
