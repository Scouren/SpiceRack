ps_card: pss and sp as a card and as a command
* settle (S3-CONTROL §2, pss and sp): -b + write block: the card deck (run) and the command deck give the same raw bytes but the Date and Title lines: pss1 65 points, pss2 10, sp2 3
V1 a 0 dc 0 sin(0 1 1k) portnum 1 z0 50
R1 a b 1k
C1 b 0 1u
V2 b 0 dc 0 portnum 2 z0 50
.pss 1k 1m b 64 10 50 1e-3
.sp lin 3 1k 3k
.control
run
print v(b)[3]
.endc
.end
