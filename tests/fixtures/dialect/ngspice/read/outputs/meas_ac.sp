meas_ac
* oracle: ngspice-45 com_measure2.c correct_vec, get_value (ac: v() is the real part, vm magnitude, vp phase in degrees, vdb dB, vr real)
.meas ac g find v(out) at=1k
.meas ac gm max vdb(out)
.meas ac ph find vp(out) at=1k
.meas ac bw when vdb(out)=-3 fall=1
.meas ac m find vm(out) when vr(out)=0.5
