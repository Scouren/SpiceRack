analyses
.param tstop=2u
.op
.dc v1 -1 1 0.5 temp 0 50 25
.ac lin 11 0 1k
.tran 1n {tstop * 2} 0 10n uic
.pz in 0 out 0 cur zer
.disto dec 5 1k 10k 0.9
.sp lin 2 1 10 1
.end
