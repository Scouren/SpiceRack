# S-NONPASSIVE-K — closed-form (the deck is ill-posed)

BUG_805_SON `mil_test1.lib`: K1 L1 L2 0.75, K2 L1 L3 0.8, L2-L3 uncoupled. The
coupling matrix [[1, .75, .8], [.75, 1, 0], [.8, 0, 1]] has determinant
1 - .75^2 - .8^2 = -0.2025: not positive definite, a non-passive circuit.
ngspice-45 stops ("timestep too small"); Xyce 7.10 integrates it.

Source: research/mismatches.md §2 (S-NONPASSIVE-K).
