param_temp_class
* oracle: R14 (a parameter reading temper, directly or through another, is the B-source parser's: inpcom.c inp_fix_temper_in_param), inpptree-parser.y (< binds tighter than ==), xpressn.c (numparam: one level)
.param t={temper} q={t==1<2}
.if (t)
.endif
