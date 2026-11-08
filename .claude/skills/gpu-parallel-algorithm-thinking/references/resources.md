# Resources

Each entry names the step it feeds.

## Reformulation

- **Blelloch, *Prefix Sums and Their Applications*** (CMU CS-90-190) —
  cs.cmu.edu/~guyb/papers/Ble93.pdf. The source for step 4: which sequential-looking problems
  reduce to scan, and the work-efficient vs span-efficient distinction used in
  `examples/prefix_scan.md`. Also the origin of the work/span accounting in `transformations.md`.
- **Karras, *Thinking Parallel*, parts I–III** (NVIDIA developer blog: collision detection, tree
  traversal, tree construction). The clearest published demonstration of step 3 — reformulating
  rather than porting, and dropping a level of dependency depth by changing what the algorithm
  computes (Morton codes and radix-tree construction in place of incremental insertion).
- **Hwu, Kirk, El Hajj, *Programming Massively Parallel Processors*, 5th ed.** Chapters on
  reduction, scan, histogram, sparse formats, merge, and graph traversal are the canonical
  treatments of the primitives in the step-5 catalogue.

## Mapping

- **CUDA C++ Programming Guide** — docs.nvidia.com/cuda/cuda-c-programming-guide/. Execution and
  memory model for step 8: warp semantics, cooperative groups, the synchronization ladder.
- **CUDA C++ Best Practices Guide** — docs.nvidia.com/cuda/cuda-c-best-practices-guide/.
  Coalescing, occupancy-in-context, transfer minimization, and the measurement discipline behind
  the step-10 reporting rules.
- **HIP programming model** — rocm.docs.amd.com/projects/HIP/en/latest/understand/programming_model.html.
  Wavefront width is 32 or 64 depending on the architecture; any warp-width assumption baked into
  a mapping is a portability bug.
- **HIP reduction tutorial** — rocm.docs.amd.com/projects/HIP/en/latest/tutorial/reduction.html.
  A worked ladder from naive to warp-primitive reduction, one factor at a time — the step-10 loop
  in miniature.

## Libraries — check before writing a primitive

- **CUB / Thrust (CCCL)** — nvidia.github.io/cccl/. Device-, block-, and warp-level scan, reduce,
  select, sort, segmented ops.
- **rocPRIM / hipCUB** — the ROCm equivalents.
- **GPU Gems 3 ch. 39, Parallel Prefix Sum with CUDA** — pedagogically useful, superseded in
  practice by CUB's single-pass decoupled look-back.
- **CUDA Samples** — github.com/NVIDIA/cuda-samples.

## Companion skill

`/data-oriented-design` — SoA vs AoS, index-based references, hot/cold splitting, arena
allocation, batch processing. Invoked at step 9, after ownership levels are fixed.

```
gpu-parallel-algorithm-thinking   what computation and dependency structure should exist
data-oriented-design              how that computation's data is represented for the hardware
```
