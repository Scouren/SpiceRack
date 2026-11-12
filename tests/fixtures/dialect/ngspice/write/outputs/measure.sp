measure
.param half=0.5
.meas tran tr trig v(a) val=0.2 rise=1 targ at=2u
.meas tran w when v(a)=half td=1u fall=last
.meas tran f find v(out) when v(in)=v(b) cross=2
.meas ac g find vdb(out) when vr(in)=0.5 rise=1
.meas dc s avg i(v1) from=0.1
.meas tran p param='tr * 2 + half'
.end
