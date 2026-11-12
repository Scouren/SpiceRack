four_gridsize
* oracle: ngspice-45 fourier.c:69-72 (the count is nfreqs alone; fourgridsize sets the interpolation grid only); the mutation law's shrunk input (seed 26): the count stays the default 9
.options fourgridsize=400
.four 1k v(out)
