max_one_arg
* oracle: settle/max_one_arg.sp (numparam's max of one argument is that argument: its first and its last, xpressn.c formula)
.func f(A,B,C,D) '2*A*sqrt(B*C*D)'
.param c='max(f(1,2,3,4))'
