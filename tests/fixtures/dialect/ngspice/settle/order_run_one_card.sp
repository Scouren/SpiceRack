o1: one .tran card, script with run
* settle (S3-CONTROL O1): ngspice-45, 'ngspice -b -r x.raw' and 'ngspice -b' with SpiceRack's appended write block
* -b -r: the tran runs at 'run' (in memory) and again after the script; the raw holds one plot, the post-script run (raw opened by ft_dorun after the script, runcoms.c dosim)
* -b + write block: plots = 'const tran1'; the post-script run does not happen (no .print/.plot/.four/.meas/.op/.tf card: main.c ft_savedotargs)
V1 a 0 PULSE(0 1 0 1u 1u 1 2)
R1 a b 1k
C1 b 0 1n
.tran 1u 5u
.control
echo "script start"
run
echo "after run"
.endc
.end
