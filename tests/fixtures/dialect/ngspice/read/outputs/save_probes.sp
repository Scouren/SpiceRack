save_probes
* oracle: ngspice-45 dotcards.c gettoks (v(a,b) is two saves; i(x) is x#branch); runs: .save v(a,c) i(v1) kept a c v1#branch, .save v(x1.n) i(v.x1.v9) @r.x1.r9[i] kept x1.n v.x1.v9#branch @r.x1.r9[i]
.save v(a) v(a, b) i(v1) v1#branch
+ @r1[i] v(x1.n) i(v.x1.v9) @r.x1.r9[i]
