expr_ops
* oracle: xpressn.c operate (% is x - y*trunc(x/y); \ is trunc(fabs(x/y))), mathfunction (nint is nearbyint, int is trunc); runs {(-7)\2} = 3, {nint(2.5)} = 2
.param a={7%3} b={(-7)\2} c={nint(2.5)} d={int(-1.5)} e={0==1<2}
