cr1: a card analysis and one run with script before and after
* settle (R85): -b: v(b)[3] = 4.499960e-07 after 'alter r1 2k' and run (the .tran card, its braces evaluated, runs at run with the script's alter in force), then 8.999840e-07 for the command tran after 'alter r1 1k' (the number fields give the same as the card)
.param tstop=1u
V1 a 0 dc 1 pulse(0 1 0 1n 1n 0.5u 1u)
R1 a b 1k
C1 b 0 1n
.tran 0.1u {tstop} 0 {tstop / 50}
.control
alter r1 2k
run
print v(b)[3]
alter r1 1k
tran 0.1u 1u 0 0.02u
print v(b)[3]
.endc
.end
