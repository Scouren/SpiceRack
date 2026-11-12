if_chain
* oracle: manual 2.15 (.if/.elseif/.else/.endif), INV-constructs B13, B28 (a single = is equality: xpressn.c fetchoperator)
.param s=2
.if (s = 1)
* one
.elseif (s == 2)
* two
.else
.endif
