alter_param_case
* oracle: names fold across the read (one spelling per name, ngspice folds case: inpcom.c:1775); a ; after the value is a comment (inpcom.c:3540; corpus measure/mos-meas-dc-control.sp)
.model nch nmos level=1
m1 d g 0 0 nch W=10u
.control
alter @m1[w]=10.01u   ; above the bin limit
altermod nch VTO=0.5
op
.endc
.end
