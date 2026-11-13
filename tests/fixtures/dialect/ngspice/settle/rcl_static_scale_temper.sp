rcl_static_scale_temper
* settle -b: 'i(v1) = -1.00000e+00' at 27 C, '-5.00000e-01' with .options temp=54: the card holds temper, so it runs as a B source whose value is the first {...} (scale's temper/27 ohm), the 1k dropped (inpcom.c inp_compat, b_transformation_wanted); 'R1 a 0 1k tc1={temper/27}' and m={temper/27} are fatal ('Undefined parameter [temper]')
v1 a 0 dc 1
R1 a 0 1k scale={temper/27}
.options temp=54
.control
op
print i(v1)
.endc
.end
