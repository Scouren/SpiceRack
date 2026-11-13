dup_def
* oracle: settle dup_subckt.sp (ngspice keeps the first .subckt), dup_func.sp (the last .func), dup_section.sp (the first .lib section): each loser changes no number, Inert R1001
.subckt cell 1 2
r1 1 2 1k
.ends
.subckt cell 1 2
r1 1 2 4k
.ends
.func f(x) {x*2}
.func f(x) {x*5}
.lib s
.param p=2
.endl
.lib s
.param p=7
.endl
