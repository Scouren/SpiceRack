# VBIC-PNP-TD — self-inconsistency and third-simulator (ngspice-45 is wrong)

ngspice-45's VBIC PNP with TD > 0 is wrong in DC: its PNP is not the mirror
of its own NPN. Reduced card `IS=1e-16 TD=2e-11`: PNP 9.361e-7 A against the
NPN mirror 2.4855e-8 A (with IS alone the two are equal); VACASK
(`vbic_1p3_4t.osdi`) gives the NPN mirror. Likely cause: the excess-phase node
voltages read without the VBICtype sign (`vbic/vbicload.c:204-205, 236-237`).

Source: research/settle-ngspice.md inv 7.3 / M4.
