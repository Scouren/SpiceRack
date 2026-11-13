dc_poi
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): 3, 1, 2, 2: '.dc v1 poi 4 3 1 2 2' keeps order and repeats
v1 1 0 1
r1 1 0 1k
.dc v1 poi 4 3 1 2 2
.end
