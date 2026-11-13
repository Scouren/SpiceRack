imp: a model and a .param read across the include, before and after it
* settle (R71 case 5): -b: i(v1) = -6.0203468e-03 (D1 with dm at 0.7 V and R1 = rv = 2k, both read before the .include): a model and a .param bind across the include
V1 n1 0 0.7
D1 n1 0 dm
R1 n1 0 {rv}
.include inc_model_param.inc
.control
set numdgt=8
op
print i(v1)
.endc
.end
