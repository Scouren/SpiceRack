lib_section
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): i(v1) = -5e-4: .lib 'file' tt reads section tt (p = 2)
.lib 'lib_section.lib' tt
v1 1 0 1
r1 1 0 'p*1k'
.dc v1 1 1 1
.end
