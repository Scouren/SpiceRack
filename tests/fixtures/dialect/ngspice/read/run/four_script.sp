four_script
* oracle: ngspice-45 fourier.c:69 (cp_getvar nfreqs: a script's set gives the count, and no item names it)
.four 1k v(out)
.control
set nfreqs=4
.endc
