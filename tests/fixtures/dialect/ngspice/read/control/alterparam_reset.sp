alterparam_reset
* oracle: S3-CONTROL S1, S2: alterparam edits the deck and takes effect at the next reset (inp.c:1735-1857; settle/state_reset.sp, state_alterparam.sp); every reset is the IR's Reset of every class (A28), the waiting alterparams right after it
.param p=1k
.control
op
alterparam p=3k
alterparam p = 4k
reset
op
reset
op
.endc
.end
