values
* oracle: inpcom.c inp_fix_param_values (a value that is no number gets braces: numparam reads it), inpeval.c INPevaluate (a card number: 10v is 10, 2mil is 2 mil), R1 (digits as written), inp_fix_temper_in_param and R14 (a value reading temper is the B-source parser's; run: .temp 27 and 127 give vto 0.527 and 0.627)
.param vt=0.45
.model m1 nmos vto={vt*2} kp=vt gamma='vt+1' lambda=-0.02 phi=2mil tox=10v
.model m2 nmos vto={0.5+temper*1m}
