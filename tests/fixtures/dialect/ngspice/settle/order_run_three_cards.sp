o3: three dot analyses, script with run twice
* settle (S3-CONTROL O3): 'run' runs every dot-card analysis, by type rank (ac1 op1 tran1); twice gives ac2 op2 tran2
* -b -r: the raw holds 3 plots (AC, OP, Tran), the post-script run only
* -b + write block: the raw holds the 6 script plots; the .op card then makes ngspice run the cards again after the script (exit 1: 'no data saved for A.C.')
V1 a 0 1
R1 a b 1k
C1 b 0 1n
.tran 1u 5u
.op
.ac dec 2 1 1k
.control
run
echo "after run1: $plots"
run
echo "after run2: $plots"
.endc
.end
