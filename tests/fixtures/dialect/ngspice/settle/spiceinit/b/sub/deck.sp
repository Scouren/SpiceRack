spiceinit_b
* settle -b (HOME empty; ngbehavior=ps linearises B exp above 14, ptfuncs.c PTexp: ps gives 8.418230e+06, C exp(20) 4.851652e+08): .spiceinit (ps) in the parent only: 4.851652e+08 run from sub/, 8.418230e+06 run from b/ as sub/deck.sp: the working directory's file is read
b1 o 0 v = exp(20)
r1 o 0 1k
.control
op
print v(o)
.endc
.end
