# CUDA ↔ HIP Portability

HIP is designed so the same source compiles for AMD (ROCm) and NVIDIA. Most CUDA code ports mechanically; the failures come from a short list of real semantic differences. Handle those deliberately.

## Contents
1. Mechanical API/keyword mapping
2. The differences that actually break things
3. Build and toolchain
4. Porting workflow with `hipify`

## 1. Mechanical API/keyword mapping

The runtime API is a near-perfect rename: the `cuda` prefix becomes `hip`.

| CUDA | HIP |
|---|---|
| `cudaMalloc` | `hipMalloc` |
| `cudaMemcpy` | `hipMemcpy` |
| `cudaMemcpyAsync` | `hipMemcpyAsync` |
| `cudaHostAlloc` | `hipHostMalloc` |
| `cudaMallocManaged` | `hipMallocManaged` |
| `cudaStreamCreate` | `hipStreamCreate` |
| `cudaDeviceSynchronize` | `hipDeviceSynchronize` |
| `cudaMemcpyToSymbol` | `hipMemcpyToSymbol` |
| `cudaGetLastError` | `hipGetLastError` |
| `cudaEventRecord` | `hipEventRecord` |

Kernel-language keywords are identical in spelling: `__global__`, `__device__`, `__host__`, `__shared__`, `__constant__`, `__restrict__`, `__syncthreads()`, `blockIdx`, `threadIdx`, `blockDim`, `gridDim`. The triple-chevron launch `kernel<<<grid, block, shmem, stream>>>(...)` works in HIP too (or use `hipLaunchKernelGGL`).

## 2. The differences that actually break things

These are where a "clean" port silently produces wrong results or half the performance:

- **Warp vs wavefront size.** NVIDIA warps are 32 threads. AMD CDNA wavefronts are **64**; RDNA can be 32 or 64. Anything that assumes 32 — warp-shuffle reduction strides, `__ballot`/`__activemask` bit widths, per-warp shared-memory partitioning, manual warp-level scans — must use `warpSize` (a built-in) or be templated. Hard-coded `32` is the number-one portability bug.
- **Shuffle and vote intrinsics.** `__shfl_*_sync` and `__ballot_sync` exist in both but mask semantics and widths differ with wavefront size. Prefer cooperative-groups-style abstractions or guard with `warpSize`.
- **LDS (shared memory) size per CU** differs from NVIDIA's shared-memory-per-SM. Tiles sized to fill an NVIDIA SM may not fit, or may leave an AMD CU underutilized. Size tiles from the target's actual capacity.
- **Register file size and occupancy math** differ per architecture (GCN/CDNA/RDNA vs NVIDIA generations). Re-tune launch bounds per target rather than copying NVIDIA-tuned numbers.
- **Atomics and floating-point details** can differ at the edges (e.g. specific atomic types, denormal handling). Validate numerics against the reference on both vendors, not just one.

## 3. Build and toolchain

- **Compilers:** `nvcc` (CUDA) vs `hipcc` (HIP). `hipcc` targets AMD via the ROCm/Clang backend and can target NVIDIA by forwarding to `nvcc`.
- **Architecture flags:** CUDA uses `-arch=sm_XX` / `-gencode`. HIP/ROCm uses `--offload-arch=gfxXXXX` (e.g. `gfx90a` for MI200-class CDNA2, `gfx942` for MI300-class). Build for the exact target arch; a mismatched arch can fall back to slow paths or fail to launch.
- **Profilers:** Nsight Compute / Nsight Systems on NVIDIA; `rocprof` / Omniperf on AMD. The metrics map onto the same concepts (occupancy, achieved bandwidth, memory vs compute bound).

## 4. Porting workflow with `hipify`

1. Run `hipify-perl` or `hipify-clang` over the CUDA sources to do the mechanical `cuda*` → `hip*` rename and produce a compile-able HIP first pass.
2. Compile with `hipcc` for the target `--offload-arch` and fix anything the translator flagged.
3. **Audit for the section-2 list by hand** — `hipify` renames APIs but does not fix hard-coded warp sizes, tile sizing, or occupancy tuning. These are semantic, not textual.
4. Validate correctness against the CPU/reference oracle on AMD hardware (not just "it compiled").
5. Re-profile and re-tune launch parameters for the AMD architecture; do not assume NVIDIA-tuned block sizes and register caps transfer.

The single best habit for portable kernels: query `warpSize` wherever the warp width matters, size on-chip memory from the device's real capacity, and keep a reference oracle so you can prove correctness on each vendor.
