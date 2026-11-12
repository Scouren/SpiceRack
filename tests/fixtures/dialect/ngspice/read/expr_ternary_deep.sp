expr_ternary_deep
* oracle: run {1?2:0?1?3:4:5} = 3 (C reads 2; numparam's then-branch rule reads 2): no single reading, INV-semantics 2.13
.param a={1?2:0?1?3:4:5}
