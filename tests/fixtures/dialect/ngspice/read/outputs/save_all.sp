save_all
* oracle: ngspice-45 outitf.c (all and allv set saveall: one set); run: .save all, .save allv and no .save keep the same vectors, every node voltage and every branch current (a b c x1.n e1#branch l1#branch v1#branch), no device current; alli alone saves nothing ('no data saved')
.save all
.save ALLV
.save alli
