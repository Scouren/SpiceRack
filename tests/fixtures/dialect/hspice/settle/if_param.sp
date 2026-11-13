if_param
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): i(v1) = -1e-3: .if (sel==1) chooses .param r=1k
.param sel=1
.if (sel==1)
.param r=1k
.else
.param r=2k
.endif
v1 1 0 1
r1 1 0 'r'
.dc v1 1 1 1
.end
