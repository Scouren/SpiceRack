ac_forms
* oracle: vsrctemp.c (ac with no magnitude is 1, no phase 0), vsrc.c (acmag, acphase), run: ac lin 1 1 1 gives 1, 2/90 deg, 3, and m 3 times ac 1 45 on the I card
V1 a 0 ac
V2 b 0 dc 1 ac 2
V3 c 0 ac 2 45
V4 d 0 acmag 2 acphase 90
V5 e 0 acphase 30
I6 f 0 c 2 m 3 ac 1 45
R6 f 0 1
.control
ac lin 1 1 1
print v(a) v(b) v(c) v(d) v(e) v(f)
.endc
.end
