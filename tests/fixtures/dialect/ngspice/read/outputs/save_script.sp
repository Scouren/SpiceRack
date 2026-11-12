save_script
* oracle: ngspice-45 breakp2.c com_save, settrace, copynode (a script's save holds from where it stands; v(a), i(x), all read as .save's)
.control
save all v(a) i(v1)
.endc
