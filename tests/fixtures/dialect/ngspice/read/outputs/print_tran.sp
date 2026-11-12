print_tran
* oracle: ngspice-45 dotcards.c fixem (v(a,b) prints v(a)-v(b), i(x) is x#branch); run: .print ac v(x1.n) i(v.x1.v9) printed both
.print tran v(a) v(a,b) i(v1) @r1[i] v(x1.n)
