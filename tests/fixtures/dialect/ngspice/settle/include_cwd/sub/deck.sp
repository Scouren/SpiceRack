r78: a deck in a subfolder that includes a sibling by its bare name
* settle (R78): run from include_cwd/ as 'ngspice -b sub/deck.sp': i(v1) = -5.00000e-04 (the sibling sub/sib.inc, 2k); with sib.inc.cwd copied to include_cwd/sib.inc, i(v1) = -2.50000e-04 (the working directory's 4k): ngspice reads a bare include name from the working directory first, then beside the deck
.include sib.inc
V1 a 0 1
.control
op
print i(v1)
.endc
.end
