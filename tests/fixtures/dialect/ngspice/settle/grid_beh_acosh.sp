grid_beh_acosh
* settle grid: values in the raw file (ngspice -b -r), compared with the IR's reference evaluator by tests/dialect_ngspice_grid.rs
va0 a0 0 dc 1.5
va1 a1 0 dc 2.5
va2 a2 0 dc 3
va3 a3 0 dc 1000.0
.nodeset v(a0)=1.5 v(a1)=2.5 v(a2)=3 v(a3)=1000.0
bd202 d202 0 v = acosh(v(a0))
bd203 d203 0 v = acosh(v(a1))
bd204 d204 0 v = acosh(v(a2))
bd205 d205 0 v = acosh(v(a3))
.op
.end
