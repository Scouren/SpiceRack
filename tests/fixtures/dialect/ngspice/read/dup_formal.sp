dup_formal
* oracle: settle dup_formal.sp (a body .param naming the formal replaces its default; an instance's value still wins): exact as the default when the value reads no name (cell: the card Inert, R1002); dup_formal_reads.sp (twin's value reads a body parameter): Unread R1002
.subckt cell 1 2 w=1
.param w=3
r1 1 2 {w}
.ends
.subckt twin 1 2 w=1
.param k=2
.param w={k*2}
r1 1 2 {w}
.ends
