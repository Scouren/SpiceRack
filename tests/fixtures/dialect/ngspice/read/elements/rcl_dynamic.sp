rcl_dynamic
* oracle: ngspice-45 inpcom.c inp_compat (an R/C/L reading v( i( temper hertz time runs as a B source: value, m, noisy kept), b_transformation_wanted; INV-semantics 6.14
R1 a b {v(a)*1k}
R2 a b r={1k+v(a)} noisy=1 m=2
L1 a b {1u*(1+i(vx))}
C1 a b {1p*(1+v(a))}
C2 a b q={1p*v(a)}
R3 a b {temper*10}
