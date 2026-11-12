temp_unusable
* oracle: ngspice-45 inp.c:1173-1182 (strtod must read the whole value, else 27 with a warning); runs: .temp 27 85 and .temp 0.1k run 27 (INV-semantics 4.02)
.temp 27 85
.temp 0.1k
