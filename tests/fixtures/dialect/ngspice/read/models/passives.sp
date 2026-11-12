passives
* oracle: inpdomod.c (r and res Resistor, c, l, sw, csw, urc, ltra, txl with R/L below 1.6e9 TransLine), res.c, cap.c, ind.c, sw.c, csw.c, urc.c, ltra.c, txl.c model tables
.model rm r rsh=50 tc1=1m
.model rp res r=2k
.model cm c cj=1e-4 cjsw=1e-10
.model lm l ind=1u
.model s1 sw vt=1 vh=0.1 ron=1 roff=1meg
.model s2 csw it=1m ih=0.1m ron=1 roff=1meg
.model u1 urc k=1.2 fmax=1g
.model t1 ltra r=0.2 l=9.13e-9 g=0 c=3.65e-12 len=1
.model x1 txl r=12.45 l=8.972e-9 g=0 c=0.468e-12 length=16
