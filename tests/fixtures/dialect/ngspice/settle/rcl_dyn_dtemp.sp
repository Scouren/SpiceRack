rcl_dyn_dtemp
* settle -b: 'i(v1) = -1.00000e-03': R={v(x)*500} with tc1=0.01 dtemp=5 is 1k, dtemp dropped by the B rewrite (inpcom.c inp_compat: value, tc1, tc2, m, noisy kept); with dtemp kept it would be 1.05k
vx x 0 dc 2
v1 a 0 dc 1
R1 a 0 R={v(x)*500} tc1=0.01 dtemp=5
.control
op
print i(v1)
.endc
.end
