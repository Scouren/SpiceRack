pmo c_literal: C1 n 0 3n cmod
* settle (lead's item 0; P1109): -b: 'Error on line 5 or its substitute:', 'c1 n 0 3n cmod', 'unknown parameter (cmod)', 'Simulation interrupted due to error!', exit 1: ngspice rejects the deck (neighbours: passive_model_order_ok.sp)
V1 n 0 dc 1 ac 1
Rn n 0 1k
C1 n 0 3n cmod
.model rmod r
.model cmod c
.model lmod l
.control
op
print i(v1)
.endc
.end
