pmok: the neighbours of passive_model_order.sp, and the text of write/elements/model_value.sp
* settle (lead's item 0; P1109): -b: mag(i(v1)) = 1.88083231e-02 and mag(i(v2)) = 3.77050060e-02 at 1 MHz (each node: two 3k resistors and 3n, hand value 1.8809e-2 with L1's -j5.3e-5, and 3.7705e-2): the value after the model runs for R, C and L, literal, braced and quoted; before it only an R's literal (R3); an R's and a C's key after the model (R4, C2). Also run apart: L1 n 0 lmod l=3m is 'unknown parameter (l)'; r=3k rmod is rejected
V1 n 0 dc 1 ac 1
R1 n 0 rmod {1k * 3}
C1 n 0 cmod 3n
L1 n 0 lmod {1m * 3}
R2 n 0 rmod 3k
R3 m 0 3k rmod
R4 m 0 rmod r={1k*3}
C2 m 0 cmod c='1n*3'
C3 m 0 cmod '1n*3'
.model rmod r
.model cmod c
.model lmod l
V2 m 0 dc 1 ac 1
.control
set numdgt=8
ac lin 1 1meg 1meg
print mag(i(v1)) mag(i(v2))
.endc
.end
