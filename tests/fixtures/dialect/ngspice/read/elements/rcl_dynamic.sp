rcl_dynamic
* oracle: ngspice-45 inpcom.c inp_compat (an R/C/L reading v( i( temper hertz time runs as a B source: value, m, noisy, tc1, tc2 kept; reciproctc=1 on the C and on the charge form, A17: run qtc at 77 C, C q={1u*v(a)} tc1=0.01 charged by 1 uA reaches 1.5 mV at 1 ms, 1.0 mV without tc: the charge divided by 1.5), b_transformation_wanted, asrcload.c:41 (tc from 300.15 K: run tc, tnom=50 at 77 C, R1 {1k+0*v(a)} tc1=0.01 is 1500 ohm, the plain R2 1270 ohm); INV-semantics 6.14; R7 is gf180mcuD smbb000149.ngspice:287
R1 a b {v(a)*1k}
R2 a b r={1k+v(a)} noisy=1 m=2
L1 a b {1u*(1+i(vx))}
C1 a b {1p*(1+v(a))}
C2 a b q={1p*v(a)}
R3 a b {temper*10}
R4 a b {v(a)*1k} tc1=1m
C3 a b {1p*v(a)} tc1=1m tc2=1u
rd2 d2 d1 'max(1e-2,(rd*nfet_10v0_asym_drdsw*(1+trth1*(temper+dtemp-25)+trth2*(temper+dtemp-25)*(temper+dtemp-25)))/(w/nf-wb)*(tanh(ra*(v(d,s)-rb*(l-lb)/(0.6e-6-lb)))))' m=nf
C4 a b q={1p*v(a)} tc1=1m
