rcl_dynamic_quoted
* oracle: ngspice-45 examples/Monte_Carlo/OpWien.sp lines 26, 31, 39 (R = '...' and C = '...' with a probe: the B-source rewrite of inp_compat); the corpus gate's legalize; ngspice lowercases first (inp_casefix), so V( is v(; the element names fold with the nets r2, c2 seen first
.param res=10k cn=1n
VR2 r2 0 0
R2 4 6  R = 'res + 0.033 * res*V(r2)'
VC2 c2 0 0
C2 6 3 C = 'cn + 0.033 * cn*V(c2)'
