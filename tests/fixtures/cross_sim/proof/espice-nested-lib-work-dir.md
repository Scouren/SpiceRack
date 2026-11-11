# ESPICE-NESTED-LIB — a nested `.lib` named from the work directory: simulator-fails (ESPice 0.3.0)

The harness writes a library the deck reaches outside its own directory to
`.cross_sim_inc/<k>-<name>` and spells every mention of it as that
work-directory name (`tests/test_cross_sim/kit.rs`), also the library's
call of its own section (`ex2.lib`: section MOS calls section RES).
HSPICE finds a nested library either way: HSPICE User Guide: Simulation
and Analysis, E-2010.12, Chapter 4, "Library Building Rules", p.104: "If
you use a relative path in a nested .LIB call, the path starts from the
directory of the parent library, not from the work directory. If the path
starts from the work directory, HSPICE can also find the library, but it
prints a warning."

ESPice 0.3.0 (VerA 0.20.0), 2026-11-11, looks from the parent library's
directory only: on the harness's layout of
`ngspice/tests/regression/lib-processing/ex2a.sp`
(`.lib '.cross_sim_inc/1-ex2.lib' MOS`, and inside that file
`.lib '.cross_sim_inc/1-ex2.lib' RES`) it stops with `Error: FileNotFound`
under `--tokenizer=hspice` and under its default reader; with the nested
name spelled from the parent (`.lib '1-ex2.lib' RES`) both run. Its own
reader runs the original, whose nested name is beside the library, so the
harness observes `target-fails`.

Decks: `lib-processing/ex2a.sp` (`ex2.lib`), `lib-processing/ex3a.sp`
(`ex3.lib` calls `extra3.lib`).

Source: ZERO-LEDGER P1-037; research/impl/evidence/P1-037/b/.
