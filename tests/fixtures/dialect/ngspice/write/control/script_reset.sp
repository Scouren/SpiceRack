script_reset
.param p=1k
.control
alter r1 2k
op
alterparam p=3k
reset
op
.endc
.end
