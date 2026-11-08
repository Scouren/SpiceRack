# Linear layer: scatter tapes, sparse LU replay, LaneLu

Read this when changing how devices stamp the matrix, how the LU is ordered, factored or refactored, or when putting independent solves in SIMD lanes. ESPice (`/home/omare/Documents/Projects/Zig/ESPice`) is the worked example throughout; file paths are relative to it.

## MNA unknowns and pattern

MNA (Nagel 1975 §3.7, eq. 3.28, crediting Ho, Ruehli & Brennan) solves

```
[ Y  B ] [ Vn ]   [ J ]
[ C  D ] [ Ib ] = [ E ]
```

with `Vn` the non-datum node voltages and `Ib` one current per voltage-defined branch. With no current-controlled branches, `B` holds only ±1 (+1 at the branch's positive node, −1 at its negative node). A voltage-source row therefore has a structural zero on the diagonal. Pivoting must keep an off-diagonal escape for those rows (threshold pivoting below), and diagonal `gmin` cannot regularize them: ngspice's `LoadGmin` (`maths/sparse/spsmp.c`) adds only to diagonal elements that exist in the structure.

The pattern is the union of every instance's structural `(row, col)` entries, built once. Datum (ground) rows and columns are not in it.

## Scatter tapes

Three u32 tapes per device batch, written once at freeze (`src/device/eval.zig` `buildTapes`):

| Tape | Index | Value |
|---|---|---|
| `gath` | `id * n_u + u` | global unknown of terminal `u` (read side: `x[gath[..]]`) |
| `rhs_idx` | `id * n_u + u` | residual row of terminal `u`; ground → trash row `n` |
| `slots` | `id * k + SlotMap.at(ru, cu)` | CSC position of local entry `(ru, cu)`; ground → trash slot `nnz` |

The value planes carry one extra cell each (`g_vals`/`c_vals` length `nnz + 1`, `rhs`/`q_vec` length `n + 1`), so a ground stamp writes the trash cell instead of branching. `k` counts only the set bits of the device's combined resistive and reactive pattern, row-major, so a structural zero has no slot, no stamp and no GPU staging cell. `SlotMap(D).at` is comptime, and stamping an entry outside the declared pattern is a compile error. The G and C planes share one slot tape, which makes the Newton matrix `G + α·C` a single axpy over `nnz`.

The runnable miniature is the last test in [`numerics.zig`](numerics.zig): two resistors, a CSC pattern, a slot tape, and a branch-free scatter whose ground terms land in the trash cell.

Accumulate with plain `+=`. On a GPU each contribution writes its own staging cell, and a reduction sums each plane cell's contributions in tape order. An atomic add would make the summation order, and so the rounding, vary between runs.

ESPice freezes the tapes, the Model/Instance PODs, the CSC pattern and the plane layout at its GPU boundary behind an ABI layout hash (`dev/devices/abi.md`). A change to a tape's meaning bumps the ABI version even when no field moves.

## Sparse LU in three phases

| Phase | When it runs | Work | Produces |
|---|---|---|---|
| Ordering | once per pattern | BTF (maximum transversal + Tarjan SCC), AMD per block | column permutation `q` |
| Full factor | first matrix; again after a failed replay | Gilbert–Peierls left-looking LU with threshold pivoting | L/U patterns, pivot sequence `pinv`, per-column topological order, `prow = pinv ∘ row_idx` |
| Refactor | every later matrix | replay the stored order: scatter values through `prow`, saxpy per stored U entry, divide L by the new pivot | new L/U values on the same pattern |

**Threshold pivoting with diagonal preference.** Keep the diagonal while `|x_kk| ≥ τ · max_i |x_i|`, else take the column maximum; τ = 1e-3 (KLU default, per ESPice `dev/solvers/klu-pipeline.md` citing the Palamadai Natarajan thesis §2.9). ngspice's `pivrel` is the same ratio (default 1e-3) with an absolute floor `pivtol` = 1e-13: allowed pivot ≥ `max(pivrel · column max, pivtol)` (ngspice 44 manual §11.1.2). The diagonal preference exists because the ordering was computed for the diagonal pivot sequence.

**Refactor replay** (`src/solver/sparse_lu.zig` `refactor`):

```
for k in 0..n:
  c = q[k]
  for p in col_ptr[c]..col_ptr[c+1]: w[prow[p]] = vals[p]   # scatter, permuted
  for p in up[k]..up[k+1]:                                  # stored topological order
    i = ui[p]; ux[p] = w[i]; w[i] = 0
    for pl in lp[i]..lp[i+1]: w[li[pl]] -= lx[pl] * ux[p]
  d = w[k]; w[k] = 0
  if d == 0 or !finite(d): fail
  cmax = max(|d|, max |w[li[p]]|)
  lx[p] = w[li[p]] / d, zeroing each slot as read
  if |d| < growth_limit * cmax: fail                        # pivot-growth monitor
```

ESPice's `growth_limit` (`refactor_growth_limit`) defaults to 1e-12; 0 disables it. The `w` workspace is all-zero between calls, kept that way by zeroing each slot as it is consumed.

**Fallback chain** (`src/solver/direct.zig`): identical values to the last factor → return at once (value compare). Same pattern → refactor. Refactor fails → full factor with fresh pivots on the same values. Full factor fails → `SingularMatrix` to the Newton caller, which answers with its own ladder (gmin, source stepping). A Jacobian known constant across iterations (a linear circuit) carries a signature (`converger.Options.matrix_sig`) that skips the factor too.

**Left out deliberately in ESPice**: row scaling and iterative refinement. They are the recovery options if a fixture shows a conditioning failure; none has.

**Where lanes do not pay inside one factor.** ESPice measured run-vectorizing the refactor's scatter axpy and lost: 4-wide runs covered 7.9% of flop-weighted elements in global coordinates, and the best restructured variant, faster in a hot microbenchmark, lost 19% wall time end-to-end (`dev/solvers/refactor-tape-2026-09.md`, i9-14900HX, Zig 0.16; not reproduced here, unverified). Factors of at most 2,048 flops now replay a flat slot tape instead of the column walk, with identical bits. The lane axis is across matrices, below.

## LaneLu: W matrices on one pivot tape

`src/solver/lane_lu.zig`: `LaneLu(W)` replays one factored `SparseLu`'s pattern and pivot sequence for `W` matrices at once, `1 ≤ W ≤ 64`, storing each structural entry as `@Vector(W, f64)`. Element `l` of every vector belongs to lane `l`'s matrix. The walks and the operation order are exactly `SparseLu.refactor`'s, so lane `l`'s factors are bitwise a scalar replay of lane `l`'s values, and `LaneLu(1)` is that oracle.

- `init(gpa, base)` borrows a factored `base` and sizes the `lx`/`ux`/`udiag`/`w`/`y` planes from it. A later full factor of `base` can change the L/U lengths; the caller then rebuilds the LaneLu.
- `refactor(src, growth_limit) u64` takes values one column at a time through `src.load(c, w, prow)`, so a source can compute them on the fly and hold no value plane (`freq_solve.zig` `Stacked`). It returns a failure mask, bit `l` for lane `l`: a zero, non-finite or decayed pivot. A failed lane's divisor is replaced by 1 (`@select`), so the surviving lanes stay exact and the failed lane's factors are garbage.
- Branches that depend on the base tape (a void pivot column, a scaled pivot) are lane-uniform by construction. A void pivot fails every lane.
- `solveT` is bitwise the scalar result per lane. `solve` has no zero-skip in either sweep, so a zero can differ in sign from `SparseLu.solve`, and a lane holding Inf can produce NaN where the scalar path skipped.
- The mask is built with `@bitCast` of the bool vector to a `W`-bit integer, then masked to `W` bits: Zig 0.16's self-hosted x86 backend widened that bitcast with a stray bit (ESPice comment; unverified on 0.17).

**Caller pattern** (`src/solver/freq_solve.zig` `solveBatch`): `(G + jωC)x = b` is solved as the real `2n` system `[G, −ωC; ωC, G]`. `W = std.simd.suggestVectorLength(f64) orelse 1` frequency points share one LaneLu pass, every right-hand side is solved per factorization (one per port for S-parameters), and any lane whose bit is set peels to the scalar per-ω path, whose full factor re-pivots.

**The differential test** (end of `lane_lu.zig`): 96 random systems of size 1–24 with a shuffled column order; lane `l` scales every entry by 1 ± 10%; some lanes get one NaN or −Inf entry. Assert the mask bit equals the scalar replay's failure, `w` is all-zero afterwards, `solveT` matches bitwise and `solve` matches by `==`. Copy this shape for any new lane kernel; the scalar oracle is the `W == 1` instantiation of the same code, never a second implementation.
