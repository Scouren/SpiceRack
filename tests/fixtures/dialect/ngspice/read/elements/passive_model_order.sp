passive_model_order
* oracle: settle/passive_model_order_*.sp (each rejected: 'unknown parameter (<model>)', an L's 'unknown parameter (l)'); settle/passive_model_order_ok.sp (the neighbours run)
R1 n 0 {1k*3} rmod
R2 n 0 '1k*3' rmod
C1 n 0 3n cmod
C2 n 0 {1n*3} cmod
L1 n 0 3m lmod
L2 n 0 lmod l=3m
R3 n 0 3k rmod
R4 n 0 rmod {1k*3}
C3 n 0 cmod 3n
L3 n 0 lmod 3m
