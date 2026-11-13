script_reset
r1 a 0 1k
.param p=1k
.control
alter r1 2k
op
alterparam p=3k
reset
op
.endc
.end
