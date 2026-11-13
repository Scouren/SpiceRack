dollar_comment
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): i(v1) = -2e-3: '$' after a blank or a number starts a comment ('1k$c' is 1k), inside a name it is a name byte (ra$b is a resistor)
v1 1 0 1 $ a comment
r1 1 0 1k$c
ra$b 1 0 1k
.dc v1 1 1 1
.end
