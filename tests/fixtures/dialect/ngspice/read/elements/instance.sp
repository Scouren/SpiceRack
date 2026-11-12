instance
* oracle: ngspice-45 inpcom.c inp_fix_inst_calls_for_numparam, found_mult_param (m is the multiplier unless the subcircuit declares m); INV-semantics 6.01; a master bound by name from the nearest scope
.subckt sub a b
.ends
.subckt subm a b params: m=1
.ends
X1 n1 n2 sub
X2 n1 n2 sub m=2 w=1
X3 n1 n2 subm m=3
X4 n1 n2 later
.subckt later a b
.ends
