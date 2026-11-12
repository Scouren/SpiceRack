reset_restores
* oracle: S3-CONTROL S1: reset rebuilds the circuit from the deck, dropping alter, altermod, script option and save (runcoms2.c:172-240; settle/state_reset.sp): Reset of every class (A28)
r1 a 0 1k
.control
alter r1 2k
op
reset
op
.endc
.end
