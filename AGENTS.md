# Project agent memory

This file is the project's committed home for project-intrinsic agent knowledge: build, test, release, architecture, and sharp-edge notes that should travel with the code.

- Add durable project-specific notes here as they are discovered through real work.
- **Walker is shared with `fd`.** `src/walker.rs` builds on `ignore::WalkBuilder`, the
  same crate `fd`/`ripgrep` use for traversal. Wall-clock parity with `fd` on I/O-bound
  real trees is expected - the traversal syscalls are identical. Real wins over `fd`
  come from the leaner per-entry hot path (no regex engine, zero-alloc SIMD matching in
  `src/matcher.rs`), not from out-tuning the walker's thread count (empirically, raising
  `WalkBuilder::threads()` above its `min(cores, 12)` default made things *worse* via
  contention - don't retry that without new evidence).
- **Benchmarking this environment is noisy.** The dev VM here regularly runs at load
  average > nproc from unrelated concurrent agent sessions. A single `hyperfine` run
  comparing two binaries back-to-back is dominated by *run order*, not the code change -
  whichever binary runs first in the sequence looks faster. To get a signal that
  survives that: interleave single-shot runs (alternate order each round) and compare
  medians, or better, sum `resource.getrusage(RUSAGE_CHILDREN)` CPU time (user+sys)
  across interleaved runs instead of wall clock - CPU time is far less sensitive to
  scheduler contention than wall time on a shared host.
- `hyperfine` and `perf` are not preinstalled and there's no sudo; `cargo install
  hyperfine --locked` works and is fast. No `perf`/`strace`/`valgrind` available and no
  way to install them without root - profiling here means reasoning from the walker/
  matcher source and validating with the CPU-time method above, not a flamegraph.

## Maintaining this file

Keep this file for knowledge useful to almost every future agent session in this project.
Do not repeat what the codebase already shows; point to the authoritative file or command instead.
Prefer rewriting or pruning existing entries over appending new ones.
When updating this file, preserve this bar for all agents and keep entries concise.
