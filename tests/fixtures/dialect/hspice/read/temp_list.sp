temp_list
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): two plots (temp=27: -5.67034677e-03, temp=50: -2.18670282e-02): '.temp 27 50' runs the analysis at each
v1 1 0 0.7
d1 1 0 dm
.model dm d is=1e-14
.option tnom=27
.temp 27 50
.dc v1 0.7 0.7 1
.end
