pwl
* oracle: vsrcpar.c VSRC_PWL, VSRC_TD, VSRC_R (r a listed time before the last; below -0.5 no repeat), vsrcload.c PWL (first value held before the first point); runs: `pwl(0 0 1u 1 r=0 td=1u)`, `td 1u r 0`, quoted values, a continuation line
.param top=2
V1 a 0 pwl(0 0 1u 1 2u 0) r=0 td=1u
V2 b 0 pwl 0 0 1u 1 2u 0 td 1u r 1u
V3 c 0 pwl(1u 2 2u '2*top')
+ ac 1
V4 d 0 pwl(0 0 1u 1) r=-1
.control
op
print v(a) v(b) v(c) v(d)
tran 0.1u 6u
print v(a)[25] v(b)[25] v(c)[25] v(a)[50] v(b)[50]
.endc
.end
