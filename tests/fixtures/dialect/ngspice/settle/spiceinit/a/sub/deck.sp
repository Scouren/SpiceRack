spiceinit_a
* settle -b (HOME empty; ngbehavior=ps linearises B exp above 14, ptfuncs.c PTexp: ps gives 8.418230e+06, C exp(20) 4.851652e+08): .spiceinit (ps) in sub/ only: 8.418230e+06 run from sub/ as deck.sp and from a/ as sub/deck.sp: the deck folder's file is read
b1 o 0 v = exp(20)
r1 o 0 1k
.control
op
print v(o)
.endc
.end
