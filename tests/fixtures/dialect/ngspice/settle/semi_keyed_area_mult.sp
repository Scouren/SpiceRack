skam: the semiconductors' area and multiplicity as the writer spells them (keyed, an expression), one card each
* settle (lead's item 0, part d): -b, run one card per deck at V1 = 0.7: D area={2*1} m='3*1' i(v1) = -3.4022081e-02 (6 x the bare card's -5.6703468e-03); J area={2*1} -4.62e-04 (2 x -2.31e-04); Z area={2*1} -1.0603704e-02 (2 x -5.3018519e-03); Q area={2*1} -2.2794794e-04 and Q m={2*1} -1.1454101e-04 (bare -5.7270504e-05); M m={2*1} at V1 = 1 -2.0000001e-05 (bare -1.0000001e-05). A D's unlabeled area after the model (D1 n 0 dmx 2, or {2*1}) is rejected ('Error on line 3', exit 1), as read_elements/semi.rs refuses; the writer spells area= only
V1 n 0 dc 0.7
D1 n 0 dmx area={2*1} m='3*1'
.model dmx d is=1e-14
.control
set numdgt=8
op
print i(v1)
.endc
.end
