# S-GAAS-STATZ-CHARGE — third-simulator (no arbiter; ngspice and Spectre within 1 %)

The Statz MESFET charge model: ngspice-45 MES level 1 against Spectre 25.1
`gaas capmod=1` and HSPICE 2026.03 NJF level 3 `sat=2 capop=1`, Cin/Cout
in pF at vds = 3 V. vgs = -1.6: ngspice 0.2547/0.1996, Spectre
0.2522/0.2000, HSPICE 0.2548/0.1996. vgs = -1.0: 0.8029/0.2293,
0.7988/0.2279, 0.8009/0.2328. vgs = -0.3: 0.9799/0.2837, 0.9763/0.2814,
0.9559/0.3108. HSPICE equals ngspice in pinch-off and the three disagree
elsewhere; the DC currents agree (3.0e-5). The ngspice/Spectre capacitance
residual is at most 1.0 % with no third simulator to say which is right:
the translation is admitted at a 1.2 % capacitance tolerance (ZERO-LEDGER
P4-024), `capmod=1` written always.

Pins: `tests/spectre/recorded/gaas.pin`, `tests/hspice/recorded/gaas.pin`
(iccad4, 2026-11-07). Source: research/settle-remote.md, row P4-024 and
"Third oracle for the model families whose numbers differ"; ZERO-LEDGER
P1-052.
