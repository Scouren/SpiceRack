wmodel4: a model parameter: {a+'b'}
* settle (audit row 4): -b: as wrap_model_3 ('Syntax error: letter [{]', 'Expression err: a+{b}}', fatal)
.param a=1 b=2
V1 n 0 1
R1 n 0 rm l=1 w=1
.model rm r rsh={a+'b'}
.control
set numdgt=17
op
print i(v1)
.endc
.end
