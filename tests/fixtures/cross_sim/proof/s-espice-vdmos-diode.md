# S-ESPICE-VDMOS-DIODE — closed-form (ESPice's VDMOS body diode)

ngspice `examples/vdmos/VDMOS-DIO.sp` and `paranoia/vdmos/VDMOS-DIO.sp.sp`
(ngspice-hspice-espice): a diode `D1` (`IS=2.13E-08 N=1.564 RS=0.0038`) and
an off VDMOS `m1` whose body diode holds the same card (`IS=2.13E-08 N=1.564
RB=0.0038`), each ramped to 0.5 V forward (`.tran 10m 2.5`), then `.ac` at
that bias. Shockley with the series resistance at 27 C (kT/q = 25.864926
mV): I = IS (exp((V - I RS)/(N kT/q)) - 1) = 4.9670526 mA, g = 1/(N kT/q /
(I + IS) + RS) = 0.12272980 S. Run 2026-11-11: ngspice-45 on the source,
and ngspice `ngbehavior=hsa` on the deck written for HSPICE, give both
devices 4.9672692 mA at t = 2.5 s and 0.12273036 S at 1 Hz (4.4e-5 and
4.5e-6 off the closed form). ESPice 0.3.0 `--tokenizer=hspice` on the
written deck gives its diode the same 4.9672692 mA and 0.12273036 S, and
its VDMOS 2.3471706 mA and 0.050513452 S; at -2 V its VDMOS conducts
1.1674e-8 A, 0.548 IS (the card's `m=0.548` is the grading coefficient).
The written deck is the circuit (the proxy reads it to the source's
numbers); the listed mismatch is ESPice's body diode, which its own diode
model does not share. The harness has no control run to class the line by:
ESPice stops on the source's `.save @d1[id]`.

Source: ZERO-LEDGER P3-064; research/impl/evidence/P3-064/espice-vdmos/.
