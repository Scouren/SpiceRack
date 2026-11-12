case_names: ngspice folds case, VACASK does not: one node spelled OUT and out, a parameter defined as Rval and read as rval, and a source named V1 read by I(v1) in a controlled source
.param Rval=2k
V1 in 0 dc 3
R1 in OUT 1k
r2 out 0 {rval}
F1 0 f v1 2
rf f 0 1k
.op
.end
