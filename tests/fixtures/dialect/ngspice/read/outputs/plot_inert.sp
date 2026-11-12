plot_inert
* oracle: ngspice-45 dotcards.c ft_cktcoms (.plot is com_asciiplot: a picture on stdout; .width sets the output width; no number)
.plot tran v(a) v(b)
.plot ac vdb(a) (0,10)
.width out=80
