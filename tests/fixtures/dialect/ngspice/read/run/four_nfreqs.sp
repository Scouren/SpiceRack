four_nfreqs
* oracle: ngspice-45 fourier.c:69 (the count is nfreqs: DC and the harmonics), A16 (no count in the row, the typed option); runs four1.cir (10 rows), four2.cir (nfreqs=3: 3 rows)
.options nfreqs=5 fourgridsize=400
.four 1k v(out)
