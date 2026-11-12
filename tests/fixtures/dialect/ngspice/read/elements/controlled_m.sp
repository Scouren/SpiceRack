controlled_m
* oracle: runs s2d at v(b)=0.5: E1 vol={v(b)*2} m=3 is 3 V (the value times m, read as a product, R31), G1 cur={v(b)} m=3 is 1.5 V into 1 ohm (the multiplicity), G/F poly with m: XSPICE Bad real value
E1 n 0 vol={v(b)*2} m=3
G1 0 n cur={v(b)} m=3
G2 0 n poly(1) b 0 0 1 m=3
F1 0 n poly(1) vs 0 1 m=3
G3 0 n b 0 1 2 m=3
