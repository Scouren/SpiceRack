defaults_duplicates
* oracle: runs s2a (C1 a 0 w=1u l=1u and L1 a 0 ic=1m show capacitance 0, inductance 0) and s2b (R1 a 0 1k r=2k is 2000, r=1k r=3k is 3000, tc1=0.01 tc1=0.02 is 0.02, X1 w=1 w=2 passes 2; run mm: M temp=50 temp=60 runs at 60: the last value wins)
.subckt sub p q params: w=5
.ends
C1 a 0 w=1u l=1u
L1 a 0 ic=1m
R1 a 0 1k r=2k
R2 a 0 r=1k r=3k
R3 a 0 1k tc1=0.01 tc1=0.02
X1 a 0 sub w=1 w=2
M1 d g 0 0 nmos temp=50 temp=60
