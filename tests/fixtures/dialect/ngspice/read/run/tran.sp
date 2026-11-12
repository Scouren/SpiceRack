tran
* oracle: ngspice-45 inp2dot.c dot_tran (tstep tstop [tstart [tmax]] [uic]), traninit.c:29-34 (no tmax: min(tstep, span/50))
.tran 1n 1u
.tran 1n 1u 100n
.tran 1n 1u 0 10n uic
.tran 10p {tstop}
