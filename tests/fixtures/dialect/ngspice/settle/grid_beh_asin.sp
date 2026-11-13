grid_beh_asin
* settle grid: values in the raw file (ngspice -b -r), compared with the IR's reference evaluator by tests/dialect_ngspice_grid.rs
va0 a0 0 dc -0.99
va1 a1 0 dc -0.5
va2 a2 0 dc -1e-07
va3 a3 0 dc 0
va4 a4 0 dc 1e-07
va5 a5 0 dc 0.5
va6 a6 0 dc 0.99
.nodeset v(a0)=-0.99 v(a1)=-0.5 v(a2)=-1e-07 v(a3)=0 v(a4)=1e-07 v(a5)=0.5 v(a6)=0.99
bd109 d109 0 v = asin(v(a0))
bd110 d110 0 v = asin(v(a1))
bd111 d111 0 v = asin(v(a2))
bd112 d112 0 v = asin(v(a3))
bd113 d113 0 v = asin(v(a4))
bd114 d114 0 v = asin(v(a5))
bd115 d115 0 v = asin(v(a6))
.op
.end
