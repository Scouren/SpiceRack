mos_levels
* oracle: inpdomod.c nmos/pmos switch (levels 0 and 1 Mos1, 2, 3, 4 BSIM1, 5 BSIM2, 6, 9, 8 and 49 BSIM3), inpfindl.c (level is (int)(value+0.5): 4.9e1 is 49, -1 is 0), b3set.c:84 (a silent BSIM3 version is 3.3.0), run: level=-1 runs level 1
.model m1 nmos vto=0.5 kp=1e-4
.model m0 nmos level=0 vto=0.5
.model mn pmos level=-1
.model m2 pmos level=2 vto=-0.5
.model m3 nmos (level=3 theta=0.1)
.model m6 nmos level=6
.model m9 pmos level=9
.model b1 nmos level=4
.model b2 nmos level=5
.model b3 nmos level=4.9e1
.model b3s nmos level=8 version=3.2.4
