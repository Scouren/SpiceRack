grid_beh_ln
* settle grid: values in the raw file (ngspice -b -r), compared with the IR's reference evaluator by tests/dialect_ngspice_grid.rs
va0 a0 0 dc 1e-07
va1 a1 0 dc 0.5
va2 a2 0 dc 1
va3 a3 0 dc 1.5
va4 a4 0 dc 2.5
va5 a5 0 dc 3
va6 a6 0 dc 1000.0
.nodeset v(a0)=1e-07 v(a1)=0.5 v(a2)=1 v(a3)=1.5 v(a4)=2.5 v(a5)=3 v(a6)=1000.0
bd42 d42 0 v = ln(v(a0))
bd43 d43 0 v = ln(v(a1))
bd44 d44 0 v = ln(v(a2))
bd45 d45 0 v = ln(v(a3))
bd46 d46 0 v = ln(v(a4))
bd47 d47 0 v = ln(v(a5))
bd48 d48 0 v = ln(v(a6))
.op
.end
