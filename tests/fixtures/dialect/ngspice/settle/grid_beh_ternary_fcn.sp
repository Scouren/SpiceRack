grid_beh_ternary_fcn
* settle grid: values in the raw file (ngspice -b -r), compared with the IR's reference evaluator by tests/dialect_ngspice_grid.rs
va0 a0 0 dc -1
va1 a1 0 dc 0
va2 a2 0 dc 0.5
va3 a3 0 dc 1
.nodeset v(a0)=-1 v(a1)=0 v(a2)=0.5 v(a3)=1
bd634 d634 0 v = ternary_fcn(v(a0), 2, 3)
bd636 d636 0 v = ternary_fcn(v(a1), 2, 3)
bd638 d638 0 v = ternary_fcn(v(a2), 2, 3)
bd640 d640 0 v = ternary_fcn(v(a3), 2, 3)
.op
.end
