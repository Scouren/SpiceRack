polarity_default
* oracle: inpgmod.c create_model (the type word is read as a parameter: psoi, phfet on MES, pmf on HFET1 are none, unrecognized, and the device runs its default n type), runs: psoi level=10 gives the nmos level=10 current -9.97549e-09
.model s1 psoi level=10
.model s2 nsoi level=57
.model h1 phfet level=1
.model f5 pmf level=5
