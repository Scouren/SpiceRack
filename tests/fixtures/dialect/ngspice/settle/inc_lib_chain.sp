ilc: section chains of one file: A calls B calls C, and D calls C too
* settle (R71, the P1015 key): -b: 'Warning: redefinition of .subckt leaf, ignored' (c read twice, through b and through d), i(v1) = -5.00000e-04: no cycle, X1 binds
V1 n1 0 1
X1 n1 0 leaf
.lib "inc_lib_chain.lib" a
.lib "inc_lib_chain.lib" d
.control
op
print i(v1)
.endc
.end
