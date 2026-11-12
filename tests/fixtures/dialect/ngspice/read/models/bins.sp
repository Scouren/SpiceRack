bins
* oracle: inpgmod.c INPgetModBin (BSIM3, BSIM4, HiSIM2, HiSIM-HV bin; misc/string.c model_name_match: base.<digits>), run: an instance of nch picks the bin by lmin lmax wmin wmax; run: rr.1 is no bin of rr (unknown parameter (rr)); A15 C4: the group nch is defined too, bound to nch.1
.model nch.1 nmos level=54 version=4.5 lmin=0 lmax=2u wmin=0 wmax=2u
.model nch.2 nmos level=54 version=4.5 lmin=2u lmax=10u wmin=0 wmax=2u
.model rr.1 r r=5
.model n9.1 nmos level=9
