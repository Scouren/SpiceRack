state_after_analysis
* oracle: R24, runs late.cir and early.cir (a .ic and .options after the .tran still hold for it)
.tran 1n 1u uic
.ic v(b)=0.5
.options method=gear
.temp 50
