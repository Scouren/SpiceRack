options
* oracle: ngspice-45 cktsopt.c:246-330 (each option, its type; itl6 is srcsteps; method trap or gear), R19 (gminfactor tunes the solver: hint), rewrite/IR-RUN.md 3 (OptionKey)
.options reltol=1e-4 abstol=1p method=gear maxord=2 noopiter itl6=20
.option gminfactor=10 savecurrents
.opt klu gshunt={g}
