names_hash
* oracle: settle/names_hash.sp (ngspice-45 runs it: a card name runs to the first blank, rs#1 and xCMP#1 are names; 2SK170 is a model name; a VDMOS's thermal nodes have no IR field yet, Unread)
.subckt cmp a b
R1 a b 2k
.ends
rs#1 a 0 1k
xCMP#1 a 0 cmp
xCMP#2 a 0 cmp
JQ1 a 0 0 2SK170
.model 2SK170 njf
MQ5 a a 0 tj tc IRFP240 thermal
.model IRFP240 vdmos
