crn: a card analysis and a .control block that never calls run
* settle (R85): -b: 'in the script', setplot lists only const (no card ran in the script), then after the block ngspice -b runs the .op card anyway (b = 0.5 printed), exit 0
V1 a 0 dc 1
R1 a b 1k
R2 b 0 1k
.op
.control
echo "in the script"
setplot
.endc
.end
