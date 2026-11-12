expr_ternary
* oracle: INV-semantics 2.13 (numparam reads a?b?c:d:e as a?c:e), run {1?0?4:8:16} = 4; else-chains group right, run {1?0:1?3:4} = 0
.param a={1?0?4:8:16} b={1?0:1?3:4}
