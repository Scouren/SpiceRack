temp_options
* oracle: ngspice-45 cktsopt.c:268-269 (temp, tnom options), inp.c:1159-1186 (.temp beats .options temp= in either order); runs temp1.cir, temp2.cir (85 over 50), .options tnom=85
.options temp=50 reltol=1e-3
.options tnom=40 temp=50
.temp 85
