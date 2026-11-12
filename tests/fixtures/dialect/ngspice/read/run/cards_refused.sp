cards_refused
* oracle: ngspice-45 inp2dot.c:962 (unimplemented dot command; no WITH_HB, no WANT_SENSE2); runs: .hb .step .sens2 .dcvolt .temperature .foo are circuit not parsed
.hb 1k 5
.step param r 1 2 1
.sens2 dc
.dcvolt v(b)=1
.temperature 85
.foo 1
