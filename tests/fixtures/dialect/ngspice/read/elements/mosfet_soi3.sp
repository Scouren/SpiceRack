mosfet_soi3
* oracle: ngspice-45 soi3.c:30 (six terminals: d fg s bg bulk thermal), inpcom.c get_number_terminals (M nodes up to the model before the first k=v from the fifth item); runs soi: the six-node card with tb tof tob runs (i(vd) = -2.001e-12), the four-node M2 card makes this build abort in spbuild.c:267 (spGetElement assertion, exit 134): read as written
M1 d g 0 bg 0 t nsoi l=1u w=10u
M2 d g 0 0 nsoi l=1u w=10u
