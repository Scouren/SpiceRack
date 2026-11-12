dc_forms
* oracle: inp2v.c (a leading value is set after INPdevParse: run `1 dc 3` op 1), inpgtok.c (`=` separates), isrc.c (`c` is dc), vsrctemp.c ("has no value, DC 0 assumed"; run op 0), runs `V1 a 0 vdd` = 2
.param vdd=2
V1 a 0 1
V2 b 0 dc 2
V3 c 0 DC=3
V4 d 0 1 dc 3
V5 e 0 dc
V6 f 0
V7 g 0 vdd
V8 h 0 {vdd*2}
.control
op
print v(a) v(b) v(c) v(d) v(e) v(f) v(g) v(h)
.endc
.end
