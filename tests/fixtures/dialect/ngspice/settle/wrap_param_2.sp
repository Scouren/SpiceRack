wparam2: a .param value: 'a'+'b'
* settle (audit row 4): -b: as wrap_param_1 ('; sign expected.', fatal)
.param a=1 b=2
.param p='a'+'b'
V1 n 0 1
R1 n 0 {p}
.control
set numdgt=17
op
print i(v1)
.endc
.end
