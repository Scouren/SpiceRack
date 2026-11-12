meas_unread
* oracle: forms the IR cannot state yet: max_at (com_measure2.c AT_MAX_AT, the position), from=/to= on a crossing (com_measure_when skips below m_from, stops past m_to), td= on a window (ignored by measure_minMaxAvg), td= outside tran (only tran_check reads m_td), par()
.meas tran a1 max_at v(a)
.meas tran a2 when v(a)=0.5 from=1u
.meas tran a3 find v(b) when v(a)=0.5 to=2u
.meas tran a4 avg v(a) td=1u
.meas ac a5 when vdb(out)=-3 td=1
.meas tran a6 find par('v(a)*2') at=1u
