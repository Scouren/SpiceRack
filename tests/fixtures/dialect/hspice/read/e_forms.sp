e_forms
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): v(a) = 6 (vcvs 3), v(b) = 8 (value='v(x)*4'), v(c) = 10 (vol='v(x)*5')
vx x 0 2
e1 a 0 vcvs x 0 3
ra a 0 1k
e2 b 0 value='v(x)*4'
rb b 0 1k
e3 c 0 vol='v(x)*5'
rc c 0 1k
.dc vx 2 2 1
.end
