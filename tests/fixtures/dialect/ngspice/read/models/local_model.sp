local_model
* oracle: inpcom.c inp_find_model (a model is looked up in its subcircuit, then the enclosing ones: nearest first), manual 2.4.3
.model dd d is=1e-14
.subckt cell a b
.model dd d is=1e-12
.ends
