# O-NG-STOPS — BUG_983_SON/mesfet: third-simulator (ngspice-45 stops)

ngspice-45 stops on the written deck: "Timestep too small; time=1e-09 …
trouble with mesmod-instance z1", so its run ends near 1 ns. VACASK
(`mes1.osdi`) and Xyce 7.10 both run the circuit to 10 µs. Two of three
run; ngspice's MESFET is the one that stops.

Source: research/settle-ngspice.md, O-NG-STOPS BUG_983 mesfet;
ZERO-LEDGER P1-045.
