rbody2
* settle -b: i(v1) = -2.00000e+05 (rb: model, l=r_l w=r_w, then r='…v()…': ngspice braces the bare l=r_l (inp_fix_param_values) and inp_compat's B source takes that first {…}: R = r_l = 1e-5), i(v2) = i(v3) = -2.81690e-02 (r= first, or a positional {…}: R = 71), i(v4) = -2.00000e+03 (no value: 1 mOhm); the gf180 kit's npolyf_u body line (sm141064.ngspice)
.param r_temp=1 r_n=10 r_rsh0=7 r_vc1=0.5 r_vc2=0 mis_r=0 sw_stat_mismatch=0 r_w=1u r_l=10u
.model npolyf_u_body r af=1.684 kf=3.6e-23 noise=1
v1 11 0 2
rb 11 0 npolyf_u_body l=r_l w=r_w
+r='r_temp*r_n*(r_rsh0+r_vc1*abs(v(11,0))/r_n)*(1+mis_r*sw_stat_mismatch)'
v2 12 0 2
rc 12 0 npolyf_u_body r='r_temp*r_n*(r_rsh0+r_vc1*abs(v(12,0))/r_n)*(1+mis_r*sw_stat_mismatch)' l=r_l w=r_w
v3 13 0 2
rd 13 0 {r_temp*r_n*(r_rsh0+r_vc1*abs(v(13,0))/r_n)*(1+mis_r*sw_stat_mismatch)}
v4 14 0 2
re 14 0 npolyf_u_body l=r_l w=r_w
.control
op
print i(v1) i(v2) i(v3) i(v4)
.endc
.end
