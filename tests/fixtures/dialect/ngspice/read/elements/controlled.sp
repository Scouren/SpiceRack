controlled
* oracle: ngspice-45 inp2e.c..inp2h.c, inpcom.c inp_compat (vcvs word, value=/vol=/cur= to B, table to XSPICE pwl input_domain=0.1), inp_poly_2g6_compat; run f5; settle table_brace_point.sp (a braced point is numparam's number, the IR's expression point is no writer's: Unread, R74 (4))
.param k=3
E1 o 0 c 0 2
E2 o 0 vcvs c 0 {k*2}
G1 o 0 c 0 1m m=3
F1 o 0 vx 2 m=2
H1 o 0 vx 1k
E3 o 0 poly(2) a 0 b 0 0 1 2
F2 o 0 poly(1) vx 0 3
G2 o 0 c 0 1 2
E4 o 0 value={v(c)*4}
E5 o 0 vol='v(c)*6'
G3 o 0 cur={v(c)/1k} m=2
E6 o 0 table {v(c)} = (0,0) (1,2)
E7 o 0 table {v(c)} = (0,0) ({k},2)
