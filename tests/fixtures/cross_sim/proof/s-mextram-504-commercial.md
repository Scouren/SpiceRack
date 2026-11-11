# S-MEXTRAM-504-COMMERCIAL — third-simulator (Spectre bjt504 and HSPICE level 6 are not 504.12.1)

Mextram at default parameters, Vce = 2 V. The CMC Mextram 504.12.1
Verilog-A (Xyce's `utils/ADMS/examples/mextram_504.12.1/bjt504.va`,
compiled by OpenVAF and run in ngspice-45 through OSDI) equals Xyce 7.10
level 504 to 6 digits: Ic at Vbe = 0.8 V is -5.18751e-4 on both. Spectre
25.1 `bjt504` (SiMKit 5.8) and HSPICE 2026.03 level 6 `vers=504` agree with
each other to 1e-3 (-3.8681e-4, -3.8714e-4) and differ from the first pair
by a constant 1.38× in low injection. Xyce is the 504.12.1 reference; the
commercial bjt504 is another model at default parameters, so Xyce 504 ↔
Spectre `bjt504` is refused (W0134) with this proof.

Pins: `tests/spectre/recorded/mextram.pin`,
`tests/hspice/recorded/mextram.pin` (iccad4, 2026-11-07). Source:
research/settle-remote.md, "Third oracle for the model families whose
numbers differ"; ZERO-LEDGER P1-052, P4-026, P4-103.
