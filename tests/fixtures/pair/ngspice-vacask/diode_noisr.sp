diode_noisr: the control of diode_isr (F31): the same reverse-biased diode without isr; passes all six (families/diode_noisr/nom_rev_dc)
.model nm d is=1e-14 n=1.05 rs=5 bv=5 ibv=1e-6 ikf=10m cjo=1p vj=0.7 m=0.4 fc=0.5 tt=1n xti=3 eg=1.11
vd d 0 dc -2.0
d1 d 0 nm area=2
.op
.end
