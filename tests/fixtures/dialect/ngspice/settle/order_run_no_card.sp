o2: no dot analysis, script with run
* settle (S3-CONTROL O2): 'run' with no dot-card analysis runs nothing: plots after run = 'const' (both modes)
V1 a 0 1
R1 a 0 1k
.control
run
echo "after run plots: $plots"
.endc
.end
