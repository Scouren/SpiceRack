# checked.sh: sourced by every run script of these fixtures (R91b). `ng DECK` and `va DECK` run one
# deck on the simulator the shell puts on PATH and leave its output in $OUT; they return 1, with
# the reason in $WHY and REFUSED set, when the run is not a source of readings: a nonzero exit
# (124 a timeout), or the simulator's own words for an analysis that did not complete. A script
# prints `refused(<why>)` in place of the reading and ends with `exit $REFUSED`.
# The words, from the runs: ngspice-45 "doAnalyses: TRAN: Timestep too small ... tran
# simulation(s) aborted" (families/diode l3_ramp_0.7, R91b), "doAnalyses: matrix is singular"
# (mos6 AC); the trunk's checked run (tests/common/sim_said.rs): ngspice "Error...",
# "Simulation interrupted", a failed measure; VACASK after "Simulating:" any line with
# "not defined", "error", "aborted", "failed".
REFUSED=0
NG_WORDS="aborted|too small|singular|^error|simulation interrupted|failed!\$|^measure '.*failed\$|no convergence"
VA_WORDS='not defined|error|aborted|failed|singular|too small'

ng() {
  OUT=$(timeout "${TMO:-60}" ngspice -b "$1" 2>&1); local rc=$?
  WHY=$(printf '%s\n' "$OUT" | sed 's/^[[:space:]]*//' | grep -iE "$NG_WORDS" | head -1)
  [ $rc -ne 0 ] && WHY="exit $rc${WHY:+: $WHY}"
  [ -z "$WHY" ] || { REFUSED=1; return 1; }
}

va() {
  OUT=$(timeout "${TMO:-60}" vacask "$1" 2>&1); local rc=$?
  WHY=$(printf '%s\n' "$OUT" | sed -n '/Simulating:/,$p' | grep -iE "$VA_WORDS" | head -1)
  [ $rc -ne 0 ] && WHY="exit $rc${WHY:+: $WHY}"
  [ -z "$WHY" ] || { REFUSED=1; return 1; }
}
