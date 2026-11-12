o4: two control blocks around cards: one script in file order?
* settle (S3-CONTROL O4): two .control blocks are one script in file order ('block one', 'block two'); an 'op' command runs only op (plots: const op1)
* -b -r: the .op card runs once more after the script (raw: 1 plot); inp.c:777-812 collects every block's lines into one list
V1 a 0 1
R1 a 0 1k
.control
echo "block one"
.endc
.op
.control
echo "block two"
op
echo "after op: $plots"
.endc
.end
