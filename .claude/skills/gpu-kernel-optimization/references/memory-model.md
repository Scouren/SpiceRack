# GPU Memory Model — Deep Dive

Covers data placement, each memory space, occupancy, register spilling, bank conflicts, and tiling. Terminology is CUDA-first with the HIP/AMD equivalent in parentheses; the model is the same on both vendors.

## Contents
1. Placement: the decision table
2. The memory spaces, ranked by latency
3. Registers and the spilling cliff
4. Shared memory / LDS and bank conflicts
5. Tiling pattern (the canonical shared-memory optimization)
6. Constant and read-only paths
7. Occupancy: what actually limits resident warps
8. Profiler symptoms → cause

## 1. Placement: the decision table

For each piece of data, ask "who reads it, how often, and is it shared?" Then place it:

| Data characteristic | Place it in | Why |
|---|---|---|
| Per-thread scratch, hot, small | Registers (just local vars) | Fastest; over-use spills to local memory (section 3). |
| Reused by many threads in a block | Shared memory (`__shared__`) / LDS | ~100× lower latency than global; the basis of tiling. |
| Read-only, broadcast to all threads | Constant memory (`__constant__`) | Cached + broadcast: one fetch serves a whole warp. |
| Large, streamed once | Global memory | The only place big arrays fit; make access coalesced. |
| Read-only, 2D spatial locality | Texture / `__ldg` read-only path | Hardware caching tuned for spatial reuse. |
| Accidental register spill | Local memory | Lives in global DRAM despite the name — a silent perf cliff. |

## 2. The memory spaces, ranked by latency

From fastest to slowest, roughly:

- **Registers** — per-thread, single-cycle. Holds your local scalar variables. Finite; the register file is split across all resident threads.
- **Shared memory (LDS on AMD)** — per-block, on-chip, tens of cycles. Software-managed scratchpad. The main tool for data reuse.
- **L1 / L2 cache** — hardware-managed. L1 is per-SM (per-CU); L2 is device-wide. You influence it through access patterns, not direct placement.
- **Constant memory** — small (64 KB on NVIDIA), cached, optimized for broadcast (all threads in a warp read the same address in one transaction).
- **Global memory (DRAM / HBM)** — hundreds of cycles of latency. Large. Everything big lives here; coalescing decides whether you use its bandwidth or waste it. A warp's accesses are served in 32/64/128-byte segments — consecutive, aligned addresses take the fewest transactions; scattered access multiplies them. Use pitched/aligned allocations for 2D data.
- **Local memory** — *not* a separate fast space. It is global DRAM used for per-thread data the compiler could not keep in registers (spills, large local arrays, indexed local arrays). Treat any local-memory traffic as a bug to investigate.

The latency gap between registers/shared and global is roughly two orders of magnitude. That gap is why "reuse data on-chip" beats "do less math" almost every time.

## 3. Registers and the spilling cliff

Each SM (CU) has a fixed register file shared by all resident threads. Registers-per-thread × threads-per-SM cannot exceed it. So a kernel that uses many registers per thread allows fewer resident warps, lowering occupancy and the GPU's ability to hide memory latency.

Worse, if a kernel needs more registers than the cap, the compiler **spills** the overflow to local memory (global DRAM). Symptoms: sudden latency, local-load/local-store traffic in the profiler.

Levers:
- Cap registers with `__launch_bounds__(maxThreadsPerBlock, minBlocksPerSM)` or the `-maxrregcount` flag to trade register count against occupancy deliberately.
- Avoid large or dynamically-indexed per-thread arrays — those cannot live in registers and become local memory.
- Shorten variable live ranges; compute-and-consume rather than holding many values live at once.

Tuning register count is a balance, not a maximize-or-minimize: more registers per thread can speed a single thread but starve occupancy. Measure both.

## 4. Shared memory / LDS and bank conflicts

Shared memory is divided into 32 equally-sized **banks**. Successive 32-bit words map to successive banks. A warp can read 32 words in one transaction *only if* each thread hits a different bank (or all read the same word — a broadcast). If two threads in the warp address different words in the same bank, the accesses **serialize** (an N-way conflict costs N times the cycles).

Common conflict: a 2D tile `float tile[32][32]` accessed column-wise. Column `c` puts every row's element in the same bank → 32-way conflict.

Fix: **pad the inner dimension** so consecutive rows land in different banks:
```cuda
__shared__ float tile[32][33];  // 33, not 32 — the pad shifts each row by one bank
```
The extra column is never used for data; it only changes the stride so column access spreads across banks.

## 5. Tiling pattern (canonical shared-memory optimization)

The reason shared memory exists: stage a block of global data on-chip once, then let every thread in the block reuse it many times. Matrix multiply is the textbook case.

Pattern:
1. Each thread block is responsible for a tile of the output.
2. Loop over the K dimension in tile-sized steps:
   - Cooperatively load one tile of A and one tile of B from global into `__shared__`.
   - `__syncthreads()` so the whole tile is present before anyone reads it.
   - Each thread accumulates partial products from the on-chip tile.
   - `__syncthreads()` before overwriting the tile next iteration.
3. Write the accumulated result to global once.

This converts O(N) global reads per element into O(N/tile) — the global traffic drops by the tile width. Always pair the two `__syncthreads()` correctly; a missing barrier is a classic race that "works" until it doesn't.

On HIP, `__shared__` and `__syncthreads()` are identical in spelling; the underlying LDS is sized per-CU and is generally comparable, but check the target architecture's LDS-per-CU when sizing tiles.

## 6. Constant and read-only paths

- **Constant memory** (`__constant__`): declare at file scope, populate from the host with `cudaMemcpyToSymbol` (`hipMemcpyToSymbol`). Best when every thread in a warp reads the *same* address (e.g. coefficients, parameters) — one fetch broadcasts to the warp. If threads read *different* constant addresses, it serializes; use global+cache instead.
- **Read-only data cache** (`__ldg` / `const __restrict__`): for large read-only inputs with spatial locality, marking pointers `const __restrict__` lets the compiler route loads through the read-only cache, easing pressure on the regular path.

## 7. Occupancy: what actually limits resident warps

Occupancy = resident warps ÷ max warps per SM. Higher occupancy gives the scheduler more warps to hide memory latency; past the point where latency is hidden, more occupancy yields nothing and can hurt (less shared memory / fewer registers per thread).

A block's residency is limited by whichever runs out first:
- Registers per thread × threads per block
- Shared memory per block
- Warp/block slots per SM

Use the occupancy calculator (or `cudaOccupancyMaxActiveBlocksPerMultiprocessor`) to see the binding constraint, then relieve *that* one — and only if latency is still exposed. Cutting shared memory to raise occupancy is a loss when that shared memory was buying reuse.

## 8. Profiler symptoms → cause

- Low achieved DRAM bandwidth **with high transaction counts**, or low global load/store efficiency → uncoalesced access → SoA layout, fix indexing (SKILL.md memory-bound; segment mechanics in section 2).
- Nonzero local-memory traffic → register spilling → reduce live registers, set launch bounds (section 3).
- Nonzero bank-conflict counter → pad the shared-memory tile (section 4).
- Low achieved occupancy with no spills → block size or shared-memory sizing caps residency (section 7).
- Low warp execution efficiency → divergence → restructure so a warp takes one path.
- Transfer time dominates the timeline → pinned memory, stream overlap, batch small copies (SKILL.md, overhead-bound).
