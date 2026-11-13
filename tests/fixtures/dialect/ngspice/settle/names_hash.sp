nh: a # in element names, a model name that starts with digits, a VDMOS with its thermal nodes
* settle (the 38 decks ngspice runs): -b: i(v1) = -2.30000e-03, exit 0: ngspice-45 runs every card here (names rs#1, xCMP#1, xCMP#2; the model 2SK170; MQ5 with its thermal nodes tj tc)
.subckt cmp a b
R1 a b 2k
.ends
V1 a 0 1
rs#1 a 0 1k
xCMP#1 a 0 cmp
xCMP#2 a 0 cmp
JQ1 a 0 0 2SK170
.model 2SK170 njf
MQ5 a a 0 tj tc IRFP240 thermal
.model IRFP240 vdmos
Rtj tj 0 1G
Rtc tc 0 1G
.control
op
print i(v1)
.endc
.end
