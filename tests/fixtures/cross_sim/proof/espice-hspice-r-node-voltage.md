# ESPICE-HSPICE-READER — a node voltage in a resistor value: simulator-fails (ESPice 0.3.0 `--tokenizer=hspice`)

ESPice's HSPICE reader stops with `UnresolvedParameter` on a resistor whose
value reads a node voltage, quoted or as `R=`; its own reader runs the
ngspice original, so the harness observes `target-fails`. The written text
is HSPICE's: HSPICE User Guide: Simulation and Analysis, E-2010.12,
Chapter 8 "Elements", p.170 `Rxxx n1 n2 [R=]'equation' …`; p.171
"user-defined equation: can be a function of any node voltages, element
currents, temperature, frequency, or time"; p.173 "Behavioral Resistors":
`R1 A B R='V(A) + I(VDD)'`.

Checked deck: `ngspice/tests/regression/misc/asrc-tc-2.sp`, written
`R3 1 3 '1k + v(9)' tc1=0.001`. ESPice 0.3.0 (VerA 0.20.0), 2026-11-11:
`--tokenizer=hspice` on the written deck: `Error: UnresolvedParameter`, no
raw file; the default reader on the original: runs. Reduced:
`R1 a 0 '1k*(1+0*abs(v(a)))'` and `R1 a 0 R='1k*(1+0*abs(v(a)))'` fail the
same way, `R1 a 0 'r*2'` (no node voltage) runs.

The group: `asrc-tc-2.sp` and the 1296 `cmcqa/gf180` decks that instantiate
a `sm141064.ngspice` subcircuit holding
`Rb 1 2 'r_temp*r_n*(r_rsh0+r_vc1*abs(v(1,2))/r_n+…)'` (540 `_sab` FET
decks, 756 resistor decks). Of the other 1820 gf180 decks of the cell none
is `target-fails`.

Source: ZERO-LEDGER P1-037 (ruling: one deck per error group checked
against the HSPICE 2010 manual, D43 b); research/impl/evidence/P1-037/;
FINDINGS-oracles "HSPICE text on ESPice" (the upstream report's text).
