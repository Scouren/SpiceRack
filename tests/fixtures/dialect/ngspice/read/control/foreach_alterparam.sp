foreach_alterparam
* oracle: S3-CONTROL S2, S5: foreach + alterparam + reset with nothing else live: one target set from the variable (settle/state_alterparam.sp)
.param p=1k
.control
foreach val 1k 3k
alterparam p=$val
reset
op
end
.endc
.end
