# Zig

## Caches

- Share `ZIG_GLOBAL_CACHE_DIR`. Keep `.zig-cache`, `zig-out`, and (Zig 0.16) unpacked `zig-pkg` directories worker-local — the shared global cache does not remove these copies, so budget them as worker outputs.
- `--maxrss` schedules steps within one build from their declared estimates; it is not a machine-wide memory limit.
- Trial incremental/watch mode only with version-specific correctness checks. Zig 0.16 documents remaining incremental miscompilations; final validation uses normal builds and tests.

## Native GC

None yet. As of 2026-10-05 both are open proposals:

- [#36858](https://codeberg.org/ziglang/zig/issues/36858): idle-time collection during `--watch`, after build updates or an idle interval, with tentative `-fcache-gc` / `-fno-cache-gc` flags.
- [#30193](https://codeberg.org/ziglang/zig/issues/30193): the underlying `zig gc` command.

Check the installed toolchain's `--help` before using either. Until one ships, the collector owns every Zig cache.

## References

[Zig 0.16 release notes](https://ziglang.org/download/0.16.0/release-notes.html) · [build scheduler](https://codeberg.org/ziglang/zig/raw/tag/0.16.0/lib/compiler/build_runner.zig)
