grid_beh_atanh
* settle grid: values in the raw file (ngspice -b -r), compared with the IR's reference evaluator by tests/dialect_ngspice_grid.rs
va0 a0 0 dc -0.99
va1 a1 0 dc -0.5
va2 a2 0 dc -1e-07
va3 a3 0 dc 0
va4 a4 0 dc 1e-07
va5 a5 0 dc 0.5
va6 a6 0 dc 0.99
.nodeset v(a0)=-0.99 v(a1)=-0.5 v(a2)=-1e-07 v(a3)=0 v(a4)=1e-07 v(a5)=0.5 v(a6)=0.99
bd206 d206 0 v = atanh(v(a0))
bd207 d207 0 v = atanh(v(a1))
bd208 d208 0 v = atanh(v(a2))
bd209 d209 0 v = atanh(v(a3))
bd210 d210 0 v = atanh(v(a4))
bd211 d211 0 v = atanh(v(a5))
bd212 d212 0 v = atanh(v(a6))
.op
.end
