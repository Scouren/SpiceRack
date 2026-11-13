num_exp_scale
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): exit 1, 'a number with both an exponent and a scale factor': 1e3k is refused
vs s 0 1
ve0 e0 0 1e3k
.dc vs 1 1 1
.end
