option_brace
* oracle: settle/option_brace.sp: a script option's value in braces is not a numparam expression (ngspice: bad type given for option, or the digits before an operator): Private, ngspice's own
.param q=50
.control
option temp={q}
op
.endc
.end
