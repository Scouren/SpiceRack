two_blocks
* oracle: S3-CONTROL O1: every block is one script in file order (inp.c:742-812; settle/order_two_blocks.sp); each block's items at its place
.control
op
.endc
.options reltol=1e-4
.control
tran 1u 5u
.endc
.end
