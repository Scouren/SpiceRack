rcl_dynamic_refused
* oracle: ngspice-45 inpcom.c inp_compat (first { on the line is the value; no brace: mal formed, exit; dtemp and ic dropped), asrcload.c:41 (tc from 300.15 K, which the IR states only on a value that reads the circuit); INV-semantics 14.13
R1 a b {temper*1k} tc1=1m
R2 a b tc1={1m} r={v(a)}
R3 a b v(a)
C1 a b {v(a)*1p} dtemp=2 ic=1
