expr_not
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): exit 1, 'cannot read the card': no unary !
vs s 0 1
ve0 e0 0 '!0'
.dc vs 1 1 1
.end
