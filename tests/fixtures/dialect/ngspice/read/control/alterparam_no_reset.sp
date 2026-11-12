alterparam_no_reset
* oracle: S3-CONTROL S2: an alterparam with no reset after it changes nothing (settle/state_reset.sp: 0.3185 before the reset): Inert
.param p=1k
.control
alterparam p=3k
op
.endc
.end
