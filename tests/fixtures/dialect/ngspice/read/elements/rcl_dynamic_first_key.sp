rcl_dynamic_first_key
* oracle: settle r_dyn_first_key.sp (ngspice braces a bare key value, l=r_l, before inp_compat takes the first {…} as the B value: R = r_l); the gf180 kit's npolyf_u body resistor
.param r_l=10u r_w=1u
.model rbody r af=1.684
rb 11 0 rbody l=r_l w=r_w
+r='7*(1+abs(v(11,0)))'
