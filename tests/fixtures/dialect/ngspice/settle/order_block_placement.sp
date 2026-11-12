p2: a control block inside a false .if branch, a subckt body, and an included file
* settle (S3-CONTROL O8): a block in a false .if branch, in a .subckt body and in an included file all run, in file order with the include inlined:
* 'in false if branch' 'in subckt' 'from include' 'top' (controls are pulled out in inp_spsource before .if and subckt expansion)
* include file inc1.inc: .control / echo "from include" / .endc
.param sel=0
V1 a 0 1
R1 a 0 1k
.if (sel == 1)
.control
echo "in false if branch"
.endc
.endif
.subckt s x
.control
echo "in subckt"
.endc
R1 x 0 1
.ends
.include inc1.inc
.control
echo "top"
.endc
.end
