in: an I source with the short noise keyword
* settle (the 38 decks, noi-ring51-demo.sp): -b: 'unknown parameter (noise)', 'Simulation interrupted due to error!', exit 1: an I card takes no short noise keyword (the deck runs because the card sits in a subcircuit nothing instantiates)
in1 0 a dc 0 noise 200u 0.05n 1.0 50u
R1 a 0 1k
.tran 1n 10n
.control
run
print v(a)[5]
.endc
.end
