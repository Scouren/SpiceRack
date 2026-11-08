---
name: gpu-kernel-optimization
description: Write GPU kernels that hit the hardware roofline instead of a hidden bottleneck. Use whenever the user wants a compute kernel or shader written, ported, or reviewed (CUDA, HIP, OpenCL, Vulkan/SPIR-V, Metal, WGSL, Triton); whenever GPU code is slow and they ask why or how to make it faster — even if they never say "kernel", e.g. "my matmul/reduction/simulation is slow on the GPU"; whenever they port kernels between NVIDIA and AMD (CUDA↔HIP, hipify, warp vs wavefront); and whenever they ask where data should live on the GPU (registers, shared/LDS, constant, global, pinned or unified host memory).
---

# GPU Kernel Optimization

Every kernel is pinned against one **ceiling** at a time — memory bandwidth, compute throughput, latency, or launch overhead — and work spent pushing on the wrong ceiling is wasted. The discipline: **name the bottleneck, compute the speed of light, close the gap.** Run these steps in order for every kernel you write or fix.

GPU kernels in Zig → the zig-gpu-kernel-engineering skill (this playbook still governs the optimization).

## Step 1 — Name the bottleneck

Place the kernel on the **roofline** before writing or changing a line of code:

- **Arithmetic intensity (AI)** = useful ops ÷ the *algorithm's minimum* DRAM traffic: each byte the algorithm cannot avoid moving, counted once per unavoidable trip. (What the current kernel *actually* moves is Step 2's job.)
- **Ridge point** = peak throughput ÷ peak bandwidth for the target GPU:

  | GPU | fp32 TFLOP/s | GB/s | ridge (FLOP/B) |
  |---|---|---|---|
  | RTX 4090 | 83 | 1008 | ~82 |
  | RTX 3090 | 36 | 936 | ~38 |
  | A100 SXM | 19.5 | 2039 | ~10 |
  | H100 SXM | 67 | 3350 | ~20 |
  | MI300X | 163 | 5300 | ~31 |

  Unknown card → ask, or state the specs you are assuming.
- AI < ridge → **memory-bound**. AI > ridge → **compute-bound**.
- Override the roofline in two cases: the grid is too small to fill the machine, or dependent memory chases (pointer walks, tiny tiles) dominate → **latency-bound**. Kernel runtimes are ~µs-scale or the timeline is mostly gaps → **overhead-bound**.

If profiler data is available (Nsight Compute, rocprof, RGP, Xcode GPU tools), trust measured SOL%/utilization over the paper estimate. Symptom→cause table for the common counters: `references/memory-model.md`.

**Done when:** the bottleneck is named as exactly one of {memory, compute, latency, overhead}, with the AI arithmetic (or profiler counters) shown.

## Step 2 — Compute the speed of light

State the best physically possible time before optimizing, so "fast" has a number:

- Memory-bound: `minimum bytes ÷ peak bandwidth`. To size the gap ("how much faster"), also estimate the *current* kernel's actual traffic: DRAM moves 32-byte sectors, so a strided AoS load drags every untouched field in the sector across the bus, and a partial-sector write costs a read-modify-write of the whole sector. `current ÷ minimum` bounds the speedup available from layout alone.
- Compute-bound: `ops ÷ peak throughput` — in the *right* units (FMA counts as 2 FLOPs; tensor/matrix cores have their own peak, often 8–16× the vector FMA peak; integer and transcendental ops have theirs).
- Latency/overhead-bound: the floor is filling the machine or amortizing launches, not a bandwidth number — state the parallelism or batching needed instead.

**Done when:** a concrete target (µs/ms, or GB/s / TFLOP/s to sustain) is written down — for memory-bound, both minimum and current traffic.

## Step 3 — Apply the playbook for that bottleneck

Optimize only against the named ceiling. Consult the matching playbook section below; also sweep the Hazards list, which applies to every kernel. For placement decisions (which memory space each buffer belongs in), use the table in `references/memory-model.md`. For anything NVIDIA↔AMD, `references/cuda-hip-portability.md`.

**Done when:** the kernel is written and every playbook item for the named bottleneck is either applied or explicitly ruled out with a reason.

## Step 4 — Verify against the speed of light

Measure (or, without hardware, estimate from the memory traffic and op counts of the final code) and compare to Step 2. Report the fraction of speed of light achieved.

- ≥ ~80% of the roofline for memory-bound, ≥ ~70% for compute-bound: done.
- Below that: the bottleneck has usually *moved* — return to Step 1 and re-classify.
- Always pair the perf claim with a correctness check against a reference implementation (bitwise for integer, tolerance for float).

**Done when:** fraction-of-roofline is reported (measured, or predicted from the final code's traffic/op counts when no hardware is available) and the correctness check passes — or, without hardware, is specified concretely (reference implementation, comparison method, tolerance).

---

## Playbook

### Memory-bound — spend every byte once, and spend it wide

- **Coalescing first.** Adjacent lanes in a warp/wavefront must touch adjacent addresses: SoA over AoS for lane-indexed data, fastest-varying index = thread index, aligned/pitched allocations for 2D.

  ```cuda
  // BAD: AoS — lane i's load of p[i].x strides by sizeof(Particle); one warp spans many lines
  struct Particle { float x, y, z, w; };
  float xi = p[i].x;

  // GOOD: SoA — a warp's 32 loads of x[i] fill contiguous 128-byte segments
  float xi = x[i];
  ```
- **Split hot from cold.** Fields the kernel never touches leave the hot arrays entirely (separate array or buffer) — deleting bytes from the stream beats caching them.
- **Vectorize loads/stores** — 128-bit accesses (`float4`, `@Vector(4, f32)`) cut instruction count and fill the bus — but only when vectorizing adds no bytes: padding `float3`→`float4` adds a third more traffic and loses on a bandwidth-bound kernel.
- **Raise AI with tiling**: stage reused data in shared/workgroup memory or registers (blocked matmul, stencil halos). Reuse is the only way to move right on the roofline.

  ```cuda
  // BAD: each thread re-reads a row of A and a column of B from DRAM — zero reuse
  for (int k = 0; k < N; ++k) acc += A[row*N + k] * B[k*N + col];

  // GOOD: stage tiles in shared memory once; each staged element is reused TILE times
  __shared__ float As[TILE][TILE + 1], Bs[TILE][TILE + 1];  // +1 pad: references/memory-model.md
  for (int t = 0; t < N; t += TILE) {
      As[ty][tx] = A[row*N + t + tx];        // cooperative, coalesced load
      Bs[ty][tx] = B[(t + ty)*N + col];
      __syncthreads();
      for (int k = 0; k < TILE; ++k) acc += As[ty][k] * Bs[k][tx];
      __syncthreads();
  }
  ```
- **Fuse kernels** that pass intermediates through DRAM; keep data in registers across the fused stages.
- Route read-only, broadcast data through **constant/read-only caches** (`__constant__`, `const __restrict__`/`__ldg`); restructure strided or random layouts (padding, Morton/tiled) so access becomes sequential.

### Compute-bound — feed the widest pipes

- Use **FMA** everywhere contractions allow; use **tensor/matrix cores** (WMMA/MFMA/cooperative matrix) for anything matmul-shaped — the vector-FMA roofline is not the real roofline if matrix units exist.
- **Lower precision** where the algorithm tolerates it (fp16/bf16/fp8/int8 with fp32 accumulate).
- **ILP:** unroll and keep independent accumulators so the FMA pipeline never stalls on a dependent chain.

  ```cuda
  // BAD: one accumulator — every FMA waits out the latency of the previous one
  float acc = 0.f;
  for (int k = 0; k < K; ++k) acc += a[k] * b[k];

  // GOOD: 4 independent chains keep the FMA pipeline full; combine once at the end
  float a0 = 0.f, a1 = 0.f, a2 = 0.f, a3 = 0.f;
  for (int k = 0; k < K; k += 4) {
      a0 += a[k+0]*b[k+0]; a1 += a[k+1]*b[k+1];
      a2 += a[k+2]*b[k+2]; a3 += a[k+3]*b[k+3];
  }
  float acc = (a0 + a1) + (a2 + a3);
  ```
- **Kill divergence**: sort/compact work so warps take uniform branches; make short branches branchless.
- Use fast-math intrinsics (`__expf`, rsqrt) when tolerance allows; transcendentals run on a narrow SFU pipe.

### Latency-bound — give the machine enough in flight

- **Occupancy is a means, not a score.** Find what caps it (registers per thread, shared memory per block, block size — mechanics in `references/memory-model.md`) and buy back only enough occupancy to hide latency; dropping registers to spill point is a loss.
- Add parallelism when the grid is small: **grid-stride loops**, split-K / split-reduction, batching independent problems into one launch.
- **Double-buffer / async copies** (`cp.async`, pipelines) so loads for tile *n+1* overlap compute on tile *n*.
- Break dependent load chains; prefetch by index instead of chasing pointers.

### Overhead-bound — stop paying per-launch and per-transfer taxes

- Fuse or batch tiny kernels; use CUDA/HIP **graphs** or persistent kernels for launch-dominated pipelines.
- **Pinned host memory + async copies on streams**, so transfers overlap compute; batch many small transfers into one.

  ```cuda
  // BAD: pageable memory, synchronous copy each iteration — the GPU idles through every transfer
  for (int i = 0; i < n; ++i) {
      cudaMemcpy(d_in, h_in + i*C, bytes, cudaMemcpyHostToDevice);
      kernel<<<g, b>>>(d_in, d_out);
  }

  // GOOD: pinned memory, two streams — copy of chunk i+1 overlaps compute of chunk i
  cudaHostAlloc(&h_in, total, cudaHostAllocDefault);
  for (int i = 0; i < n; ++i) {
      cudaMemcpyAsync(d_in[i%2], h_in + i*C, bytes, cudaMemcpyHostToDevice, s[i%2]);
      kernel<<<g, b, 0, s[i%2]>>>(d_in[i%2], d_out[i%2]);
  }
  ```
- Unified/managed memory (`cudaMallocManaged`/`hipMallocManaged`) hides page migration; when using it, prefetch (`cudaMemPrefetchAsync`) and add access hints.
- Eliminate hidden host↔device syncs (per-iteration readbacks, implicit sync copies); keep data resident on the GPU across the whole pipeline — transfer once in, once out.

### Hazards — check on every kernel

- **Shared memory bank conflicts**: pad the tile's inner dimension or swizzle — mechanics and the canonical pad in `references/memory-model.md`.
- **Atomics contention**: privatize per block, then reduce — funneling a whole grid into one address serializes it.

  ```cuda
  // BAD: every thread in the grid hammers the same global counters — fully serialized
  atomicAdd(&hist[bin], 1);

  // GOOD: privatize into shared memory; contention stays inside the block, one flush at the end
  __shared__ unsigned s_hist[BINS];
  atomicAdd(&s_hist[bin], 1);
  __syncthreads();
  if (threadIdx.x < BINS) atomicAdd(&hist[threadIdx.x], s_hist[threadIdx.x]);
  ```
- **Register spilling**: watch registers-per-thread; spills turn a compute kernel memory-bound (`references/memory-model.md`).
- **Misaligned or partial cache-line access**: align array bases and row pitches to the load width; when elements straddle sectors on a memory-bound kernel, reshape to SoA rather than pad elements — padding adds traffic.
- **Hard-coded warp size**: a literal `32` in shuffle masks, reduction strides, or per-warp sizing — use `warpSize` or a template parameter; AMD wavefronts differ (`references/cuda-hip-portability.md`).
- **Block size**: a multiple of the warp/wavefront size; 128–256 is the sane default to tune from.

---

## References

- `references/memory-model.md` — memory-placement decision table, the spaces ranked by latency, register spilling, bank conflicts, tiling mechanics, occupancy limits, profiler symptom→cause.
- `references/cuda-hip-portability.md` — CUDA↔HIP API mapping, warp/wavefront sizes, `nvcc`/`hipcc` flags, `hipify` porting workflow.
