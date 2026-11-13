control_in_section
* settle -b: 'lib-block-ran' then 'deck-block', 'v(a) = 1', and set lists noinit: the .control block in the called section (control_in_section.lib) runs, before the deck's
.lib "control_in_section.lib" sec
v1 a 0 dc 1
r1 a 0 1k
.control
echo deck-block
op
print v(a)
show noinit
set
.endc
.end
