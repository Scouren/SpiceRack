welement2: an element value: 'a'+'b'
* settle (audit row 4): -b: as wrap_element_1 ('unknown parameter (+)'): each '…' is a {…} of its own
.param a=1 b=2
V1 n 0 1
R1 n 0 'a'+'b'
.control
set numdgt=17
op
print i(v1)
.endc
.end
