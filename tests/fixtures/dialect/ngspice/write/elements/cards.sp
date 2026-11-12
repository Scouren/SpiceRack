cards
R1 a b 1k rmod tc1=1m temp=50 noisy=0 l=1u m=2
C1 a 0 1p ic=0.5
R2 a b {v(a) * 1k}
C2 a b q={1p * v(a)}
K1 L1 L2 0.5
E1 o 0 c 0 2
F1 o 0 vx 2 m=2
E3 o 0 poly(2) a 0 b 0 0 1 2
B1 b 0 i={v(a) * 2} m=2
E4 o 0 vol={v(c) * 4}
E6 o 0 table {v(c)} = (0, 0) (1, 2)
D1 a k dmod area=2 off ic=0.6
M1 d g s b nmos ic=1,2,3 w=1u m=2
W1 a b vctl wmod off
T2 a 0 b 0 z0=75 f=1g nl=0.5
U1 a b 0 umod l=1m n=5
X1 a b sub m=2
A1 in out amod
.end
