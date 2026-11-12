pmo l_key: L1 n 0 lmod l=3m
* settle (lead's item 0; P1109): -b: 'Error on line 5 or its substitute:', 'l1 n 0 lmod l=3m', 'unknown parameter (l)', 'Simulation interrupted due to error!', exit 1: ngspice rejects the deck (neighbours: passive_model_order_ok.sp)
V1 n 0 dc 1 ac 1
Rn n 0 1k
L1 n 0 lmod l=3m
.model rmod r
.model cmod c
.model lmod l
.control
op
print i(v1)
.endc
.end
