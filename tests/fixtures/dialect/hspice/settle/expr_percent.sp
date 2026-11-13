expr_percent
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): exit 1, 'cannot read the card': no % operator
vs s 0 1
ve0 e0 0 '7%3'
.dc vs 1 1 1
.end
