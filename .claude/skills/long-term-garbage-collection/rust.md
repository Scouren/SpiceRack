# Rust

## Caches

- Reuse Cargo's dependency cache. Keep toolchain, target, profile, and feature selections consistent across agents where the project allows; each distinct combination is another set of artifacts.
- When full debugger information is unnecessary, merge these into the existing workspace-root profiles:

  ```toml
  [profile.dev]
  debug = "line-tables-only"

  [profile.dev.package."*"]
  debug = false
  ```

- Use `RUSTC_WRAPPER=sccache` when compatible. Give it a budget (`SCCACHE_CACHE_SIZE=10G`), run one server per local cache directory, and inspect `sccache --show-stats`. That budget does not cap target directories.
- Keep incremental compilation for warm edit loops unless measurements favor disabling it. Trial `CARGO_INCREMENTAL=0` for disposable builds: sccache cannot cache incremental compilations or final executable linking.
- Verify cross-worktree cache hits. Released sccache 0.18.0 lacks Rust support for `SCCACHE_BASEDIRS`; check newer releases before relying on it.

## Native GC

- Cargo ≥ 1.88 auto-cleans its global cache (registry indexes, crate files, git checkouts): network files unused for 3 months, generated files unused for 1 month. Tune with `cache.auto-clean-frequency`; it skips `--offline` and `--frozen` runs.
- Target directories have no native GC ([cargo#13136](https://github.com/rust-lang/cargo/issues/13136)); the collector owns them.

## References

[Cargo performance](https://doc.rust-lang.org/cargo/guide/build-performance.html) · [Cargo cache config](https://doc.rust-lang.org/cargo/reference/config.html#cache) · [sccache Rust limits](https://github.com/mozilla/sccache/blob/main/docs/Rust.md) · [sccache local cache](https://github.com/mozilla/sccache/blob/main/docs/Local.md) · [Rust path normalization](https://github.com/mozilla/sccache/pull/2794)
