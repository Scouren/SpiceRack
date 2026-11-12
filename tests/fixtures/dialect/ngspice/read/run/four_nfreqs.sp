four_nfreqs
* oracle: ngspice-45 fourier.c:69 (the harmonic count is the deck's nfreqs option: copying it repeats an operand, R13 b)
.options nfreqs=5
.four 1k v(out)
