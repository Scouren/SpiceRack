script_vector
* oracle: S3-CONTROL V3: alter $&k reads a vector reprinted with six digits (variable.c:1045; settle/subst.sp): Script
r1 a 0 1k
.control
let k = 1.23456789k
alter r1 $&k
op
.endc
.end
