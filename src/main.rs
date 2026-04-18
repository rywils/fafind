use clap::{ArgGroup, Parser};
use ignore::WalkBuilder;
use smallvec::SmallVec;
use std::ffi::OsStr;
use std::io::{BufWriter, Write};
use std::cell::UnsafeCell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

// On Unix, OsStrExt::as_bytes() gives direct access to the raw filename bytes
// without UTF-8 conversion, used for zero-copy output.
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

const AFTER_HELP: &str = r#"Matching modes (default: stem — strips extension, exact stem match):
  fafind main .            stem: finds main.rs, main.go — not domain.rs
  fafind -s foo /home      substr: finds foobar, foo.txt, prefoo
  fafind -p Makefile /etc  exact: full filename must match literally

Other examples:
  fafind -i README .       case-insensitive stem match
  fafind --max-depth 3 main .
  fafind --exclude target,node_modules main .
  fafind --gitignore src .
"#;

// Per-worker output accumulator. Workers append raw match bytes here;
// the Vec is moved into the shared collector on drop — zero copies.
const WORKER_BUF_CAP: usize = 256 * 1024; // 256 KB initial capacity per worker

// BufWriter capacity for the final sequential stdout write in main.
const WRITER_BUF_CAP: usize = 256 * 1024;

#[derive(Parser, Debug)]
#[command(name = "fafind")]
#[command(version = "0.1.0")]
#[command(about = "Fast filesystem search by filename")]
#[command(after_help = AFTER_HELP)]
#[command(group(ArgGroup::new("mode").args(["substr", "precise"]).multiple(false)))]
struct Cli {
    target: String,

    #[arg(value_name = "ROOT")]
    root: Option<PathBuf>,

    /// Substring match: find files whose name contains TARGET anywhere
    #[arg(short = 's', long, group = "mode")]
    substr: bool,

    /// Exact match: full filename must equal TARGET (including extension)
    #[arg(short = 'p', long, group = "mode")]
    precise: bool,

    /// Print every scanned file
    #[arg(short = 'v', long)]
    verbose: bool,

    /// Case-insensitive matching
    #[arg(short = 'i', long)]
    ignore_case: bool,

    /// Filter by entry type: f = files only, d = directories only
    #[arg(long = "type", value_name = "f|d")]
    entry_type: Option<String>,

    /// Separate output with NUL instead of newline (for xargs -0)
    #[arg(short = '0', long = "null")]
    null: bool,

    /// Maximum directory depth to recurse into
    #[arg(long, value_name = "N")]
    max_depth: Option<usize>,

    /// Comma-separated list of directory names to exclude
    #[arg(long, value_name = "DIRS", value_delimiter = ',')]
    exclude: Vec<String>,

    /// Respect .gitignore files
    #[arg(long)]
    gitignore: bool,

    /// Suppress the summary line (scanned/found/elapsed)
    #[arg(short = 'q', long)]
    quiet: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MatchMode {
    Substr,
    Precise,
    Standard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryType {
    Any,
    File,
    Dir,
}

// ---------------------------------------------------------------------------
// Preprocessed match target
// ---------------------------------------------------------------------------

/// Holds the pre-processed target for matching.
/// Constructed once; shared read-only across all worker threads via Arc.
#[derive(Clone)]
struct MatchTarget {
    /// Canonical bytes: lowercased if ignore_case, raw otherwise.
    /// Stored as Arc<[u8]> to avoid the extra vtable of Arc<str> and enable
    /// direct byte-slice access without going through str.
    canonical: Arc<[u8]>,
    /// Length of canonical, cached to avoid a pointer deref in the hot loop.
    canonical_len: usize,
    mode: MatchMode,
    ignore_case: bool,
    /// True if the target contains only ASCII bytes.
    target_is_ascii: bool,
    /// True if the needle is short enough (≤ 4 bytes) for the sliding-window
    /// fast path in substr mode, avoiding the stack-buffer copy entirely.
    short_needle: bool,
}

impl MatchTarget {
    fn new(raw: &str, mode: MatchMode, ignore_case: bool) -> Self {
        let canonical: Arc<[u8]> = if ignore_case {
            raw.to_ascii_lowercase().into_bytes().into()
        } else {
            raw.as_bytes().into()
        };
        let target_is_ascii = raw.is_ascii();
        let canonical_len = canonical.len();
        let short_needle = canonical_len <= SHORT_NEEDLE_THRESHOLD;
        Self { canonical, canonical_len, mode, ignore_case, target_is_ascii, short_needle }
    }

    /// Hot path: returns true if `filename` matches this target.
    ///
    /// ASCII fast-path (covers >95% of real-world filenames):
    ///   - Detects ASCII-only filenames with a SIMD-friendly all-< 128 check.
    ///   - Performs case folding inline with `to_ascii_lowercase()` on a
    ///     stack-allocated copy — zero heap allocation.
    ///
    /// Unicode fallback:
    ///   - Only triggered when a byte ≥ 128 is present in the filename.
    ///   - Falls back to `to_lowercase()` which may allocate, but this is the
    ///     rare case and correctness requires it.
    #[inline(always)]
    fn is_match(&self, filename: &OsStr) -> bool {
        let bytes = filename.as_encoded_bytes();

        match self.mode {
            MatchMode::Precise => self.match_precise(bytes),
            MatchMode::Substr => self.match_substr(bytes),
            MatchMode::Standard => self.match_standard(bytes),
        }
    }

    #[inline(always)]
    fn match_precise(&self, bytes: &[u8]) -> bool {
        if !self.ignore_case {
            return bytes == self.canonical.as_ref();
        }
        if bytes.len() != self.canonical_len {
            return false;
        }
        if self.target_is_ascii {
            ascii_eq_ignore_case_single_pass(bytes, &self.canonical)
        } else {
            unicode_eq_ignore_case(bytes, &self.canonical)
        }
    }

    #[inline(always)]
    fn match_substr(&self, bytes: &[u8]) -> bool {
        if !self.ignore_case {
            // Case-sensitive: SIMD memmem, zero allocation.
            return memchr::memmem::find(bytes, &self.canonical).is_some();
        }
        if self.target_is_ascii {
            // ASCII/Unicode decision made ONCE here, never inside inner loops.
            // bytes.is_ascii() is a tight all-< 128 scan the compiler
            // auto-vectorizes; it runs once and gates the entire hot path.
            if bytes.is_ascii() {
                if self.short_needle {
                    ascii_substr_short(bytes, &self.canonical)
                } else {
                    ascii_contains_ignore_case(bytes, &self.canonical)
                }
            } else {
                unicode_contains_ignore_case(bytes, &self.canonical)
            }
        } else {
            unicode_contains_ignore_case(bytes, &self.canonical)
        }
    }

    #[inline(always)]
    fn match_standard(&self, bytes: &[u8]) -> bool {
        let stem = stem_bytes(bytes);
        if !self.ignore_case {
            return stem == self.canonical.as_ref();
        }
        if stem.len() != self.canonical_len {
            return false;
        }
        if self.target_is_ascii {
            ascii_eq_ignore_case_single_pass(stem, &self.canonical)
        } else {
            unicode_eq_ignore_case(stem, &self.canonical)
        }
    }
}

// ---------------------------------------------------------------------------
// Case-folding helpers
// ---------------------------------------------------------------------------

// FILENAME_BUF_LEN: stack buffer size for ASCII case-folding in substr mode.
//
// POSIX NAME_MAX = 255 on Linux/macOS/BSDs; +1 for headroom.
// Windows MAX_PATH component: 255 UTF-16 chars × up to 3 UTF-8 bytes = 765.
#[cfg(unix)]
const FILENAME_BUF_LEN: usize = 256; // NAME_MAX (255) + 1
#[cfg(not(unix))]
const FILENAME_BUF_LEN: usize = 768;

// Needle length threshold below which the manual sliding-window path is used
// instead of stack-copy + memmem. Tune by benchmarking both paths.
// At ≤4 bytes the window loop beats memmem due to zero setup cost;
// above this memmem's SIMD vectorization dominates.
const SHORT_NEEDLE_THRESHOLD: usize = 4;

/// Single-pass ASCII case-insensitive equality.
///
/// One loop: non-ASCII detection + lowercased compare simultaneously.
/// No separate is_ascii() pre-scan.
#[inline(always)]
fn ascii_eq_ignore_case_single_pass(bytes: &[u8], canonical: &[u8]) -> bool {
    let n = bytes.len(); // caller checked len equality
    let mut i = 0usize;
    while i < n {
        let a = bytes[i];
        if a >= 128 {
            return unicode_eq_ignore_case(bytes, canonical);
        }
        if a.to_ascii_lowercase() != canonical[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Short-needle ASCII case-insensitive substring search.
///
/// PRECONDITION: caller has confirmed haystack.is_ascii() == true.
/// No >= 128 checks inside — the hot loops are branch-free straight-line
/// compares: only the window-advance test and one equality chain per position.
///
/// Manual indexed outer loop (no fat pointer from .windows()).
/// Inner loop unrolled per nlen via match: no inner loop at all,
/// exposes ILP, lets LLVM use cmov for the equality chain.
/// Bounds on haystack[i+1..i+nlen] are provably in-range given
/// i <= limit = hlen - nlen, so LLVM eliminates those checks.
#[inline(always)]
fn ascii_substr_short(haystack: &[u8], needle: &[u8]) -> bool {
    let hlen = haystack.len();
    let nlen = needle.len();
    if hlen < nlen {
        return false;
    }
    let limit = hlen - nlen;
    match nlen {
        1 => {
            let n0 = needle[0];
            let mut i = 0usize;
            while i <= limit {
                if haystack[i].to_ascii_lowercase() == n0 { return true; }
                i += 1;
            }
        }
        2 => {
            let (n0, n1) = (needle[0], needle[1]);
            let mut i = 0usize;
            while i <= limit {
                if haystack[i].to_ascii_lowercase() == n0
                    && haystack[i + 1].to_ascii_lowercase() == n1
                {
                    return true;
                }
                i += 1;
            }
        }
        3 => {
            let (n0, n1, n2) = (needle[0], needle[1], needle[2]);
            let mut i = 0usize;
            while i <= limit {
                if haystack[i].to_ascii_lowercase() == n0
                    && haystack[i + 1].to_ascii_lowercase() == n1
                    && haystack[i + 2].to_ascii_lowercase() == n2
                {
                    return true;
                }
                i += 1;
            }
        }
        4 => {
            let (n0, n1, n2, n3) = (needle[0], needle[1], needle[2], needle[3]);
            let mut i = 0usize;
            while i <= limit {
                if haystack[i].to_ascii_lowercase() == n0
                    && haystack[i + 1].to_ascii_lowercase() == n1
                    && haystack[i + 2].to_ascii_lowercase() == n2
                    && haystack[i + 3].to_ascii_lowercase() == n3
                {
                    return true;
                }
                i += 1;
            }
        }
        _ => return true, // nlen == 0: degenerate, always matches
    }
    false
}

/// ASCII case-insensitive substring search using a stack buffer + memmem.
///
/// PRECONDITION: caller has confirmed haystack.is_ascii() == true.
/// No >= 128 guard inside the fill loop — pure lowercase + copy,
/// no branches other than the loop counter. memmem then runs SIMD over
/// the contiguous lowercased buffer.
#[inline(always)]
fn ascii_contains_ignore_case(haystack: &[u8], needle: &[u8]) -> bool {
    if haystack.len() <= FILENAME_BUF_LEN {
        let mut buf = [0u8; FILENAME_BUF_LEN];
        let h = &mut buf[..haystack.len()];
        for (dst, &src) in h.iter_mut().zip(haystack.iter()) {
            *dst = src.to_ascii_lowercase();
        }
        memchr::memmem::find(h, needle).is_some()
    } else {
        // Practically unreachable on any real filesystem (exceeds NAME_MAX).
        let lower: Vec<u8> = haystack.iter().map(|b| b.to_ascii_lowercase()).collect();
        memchr::memmem::find(&lower, needle).is_some()
    }
}

/// Unicode case-insensitive equality check (cold path).
#[cold]
fn unicode_eq_ignore_case(bytes: &[u8], canonical_lower: &[u8]) -> bool {
    let Ok(s) = std::str::from_utf8(bytes) else { return false };
    s.to_lowercase().as_bytes() == canonical_lower
}

/// Unicode case-insensitive substring search (cold path).
#[cold]
fn unicode_contains_ignore_case(bytes: &[u8], needle_lower: &[u8]) -> bool {
    let Ok(s) = std::str::from_utf8(bytes) else { return false };
    let lower = s.to_lowercase();
    memchr::memmem::find(lower.as_bytes(), needle_lower).is_some()
}

/// Extract the file stem as a byte slice from raw filename bytes.
/// Equivalent to Path::file_stem() but zero-allocation.
/// Manual reverse scan replaces iterator state machine from rposition.
#[inline(always)]
fn stem_bytes(bytes: &[u8]) -> &[u8] {
    let n = bytes.len();
    if n == 0 { return bytes; }
    let mut i = n;
    while i > 1 { // stop at 1: dot at index 0 = hidden file, stem = whole name
        i -= 1;
        if bytes[i] == b'.' {
            return &bytes[..i];
        }
    }
    bytes
}

// ---------------------------------------------------------------------------
// End-of-run totals — written once per thread on drop, read once in main
// ---------------------------------------------------------------------------

// Written once per thread on drop, read once in main — never in hot path.
type Totals = Arc<Mutex<(u64, u64)>>; // (scanned, found)

// Lock-free output slot array.
//
// Layout: (next_slot_idx, [UnsafeCell<Option<Vec<u8>>>; num_workers])
//
// Safety contract:
//   - Each worker claims a unique slot index via fetch_add ONCE at construction,
//     before processing any entry. No two workers share a slot index.
//   - A worker writes to its slot ONLY in Drop, after all entry processing is done.
//   - Main reads slots ONLY after walk_parallel() returns, which blocks until
//     every worker thread has exited (and therefore dropped WorkerState).
//   - Therefore: no concurrent access to any slot ever occurs.
struct OutputSlots {
    next_idx: AtomicUsize,
    slots: Vec<UnsafeCell<Option<Vec<u8>>>>,
}

// SAFETY: Workers only write to their own unique slot; main reads only after
// all workers are done. Unique ownership per slot guarantees no data races.
unsafe impl Sync for OutputSlots {}
unsafe impl Send for OutputSlots {}

impl OutputSlots {
    fn new(capacity: usize) -> Arc<Self> {
        let mut slots = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            slots.push(UnsafeCell::new(None));
        }
        Arc::new(Self { next_idx: AtomicUsize::new(0), slots })
    }

    // Claim the next available slot index. Called once per worker at construction.
    fn claim(&self) -> usize {
        self.next_idx.fetch_add(1, Ordering::Relaxed)
    }

    // Write buf into slot. SAFETY: caller must hold the unique index for this slot.
    unsafe fn write(&self, idx: usize, buf: Vec<u8>) {
        if idx < self.slots.len() {
            unsafe { *self.slots[idx].get() = Some(buf); }
        }
    }

    // Drain all slots in index order. Called by main after all workers have exited.
    fn drain_ordered(self: Arc<Self>) -> impl Iterator<Item = Vec<u8>> {
        // Unwrap the Arc — main is the only holder at this point.
        let inner = Arc::try_unwrap(self)
            .unwrap_or_else(|_| panic!("OutputSlots Arc still shared after walk"));
        inner.slots.into_iter().filter_map(|cell| cell.into_inner())
    }
}

// ---------------------------------------------------------------------------
// Walk configuration — immutable after construction, shared via Arc
// ---------------------------------------------------------------------------

// Exclude lists are typically 0–8 entries. SmallVec<[_; 8]> keeps them on
// the stack, avoiding a heap allocation AND replacing the hash-table lookup
// with a linear scan that is faster for n < ~16 due to cache locality.
type ExcludeList = SmallVec<[Box<[u8]>; 8]>;

struct WalkConfig {
    target: MatchTarget,
    max_depth: Option<usize>,
    /// Excluded directory names as raw byte slices for zero-copy linear scan.
    exclude: Arc<ExcludeList>,
    entry_type: EntryType,
    null_terminate: bool,
    gitignore: bool,
    verbose: bool,
}

// ---------------------------------------------------------------------------
// Hot-path entry processor
// ---------------------------------------------------------------------------

/// Append a matched path to a raw byte buffer.
///
/// On Unix: appends the raw OS bytes via OsStrExt::as_bytes() — no UTF-8
/// validation, no format string, just a slice extend.
/// On non-Unix: falls back to path.display().
#[inline(always)]
fn append_path(buf: &mut Vec<u8>, path: &Path, null_terminate: bool) {
    #[cfg(unix)]
    {
        buf.extend_from_slice(path.as_os_str().as_bytes());
        if null_terminate { buf.push(b'\0'); } else { buf.push(b'\n'); }
    }
    #[cfg(not(unix))]
    {
        let s = if null_terminate {
            format!("{}\0", path.display())
        } else {
            format!("{}\n", path.display())
        };
        buf.extend_from_slice(s.as_bytes());
    }
}

/// Worker state: entirely private per-thread.
/// Zero shared-memory writes in the hot path.
struct WorkerState {
    config: Arc<WalkConfig>,
    local_scanned: u64,
    local_found: u64,
    /// Accumulated match output. Grown inline; written to slot on drop.
    out_buf: Vec<u8>,
    /// Lock-free output slot array + this worker's unique slot index.
    slots: Arc<OutputSlots>,
    slot_idx: usize,
    totals: Totals,
}

impl WorkerState {
    fn new(config: Arc<WalkConfig>, slots: Arc<OutputSlots>, totals: Totals) -> Self {
        let slot_idx = slots.claim(); // fetch_add once, before any hot-path work
        Self {
            config,
            local_scanned: 0,
            local_found: 0,
            out_buf: Vec::with_capacity(WORKER_BUF_CAP),
            slots,
            slot_idx,
            totals,
        }
    }
}

impl Drop for WorkerState {
    fn drop(&mut self) {
        let buf = std::mem::take(&mut self.out_buf);
        if !buf.is_empty() {
            // SAFETY: slot_idx is unique to this worker; no other thread writes here.
            // main reads only after walk_parallel() returns (all workers dropped).
            unsafe { self.slots.write(self.slot_idx, buf); }
        }
        if let Ok(mut t) = self.totals.lock() {
            t.0 += self.local_scanned;
            t.1 += self.local_found;
        }
    }
}

/// Hot path: called for every filesystem entry.
/// Zero syscalls, zero heap allocations, zero shared-memory writes.
#[inline(always)]
fn process_entry(path: &Path, is_dir: bool, state: &mut WorkerState) {
    state.local_scanned += 1;

    // Entry-type filter: cheapest rejection possible.
    // is_dir is free from readdir d_type — no stat syscall.
    match state.config.entry_type {
        EntryType::File if is_dir => return,
        EntryType::Dir if !is_dir => return,
        _ => {}
    }

    // file_name() is a O(1) reverse scan of path bytes — no allocation.
    let Some(filename) = path.file_name() else { return };

    // Hoist config fields to locals: avoids repeated Arc ptr deref through
    // state.config.<field> on every access in the hot match + output path.
    let cfg = &*state.config;

    if cfg.verbose {
        verbose_scan(path);
    }

    if cfg.target.is_match(filename) {
        state.local_found += 1;
        if cfg.verbose {
            let s = format!("[MATCH] {}\n", path.display());
            state.out_buf.extend_from_slice(s.as_bytes());
        } else {
            append_path(&mut state.out_buf, path, cfg.null_terminate);
        }
    }
}

/// Verbose scan log — cold path, out-of-line to keep process_entry tight.
#[cold]
#[inline(never)]
fn verbose_scan(path: &Path) {
    let _ = eprintln!("[SCAN] {}", path.display());
}

/// Returns true if this directory should be skipped.
/// Linear scan over SmallVec — faster than HashSet for n ≤ ~16.
#[inline(always)]
fn should_skip_dir(path: &Path, exclude: &ExcludeList) -> bool {
    if exclude.is_empty() {
        return false;
    }
    path.file_name()
        .map(|n| {
            let b = n.as_encoded_bytes();
            exclude.iter().any(|e| e.as_ref() == b)
        })
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Walker
// ---------------------------------------------------------------------------

/// Single unified parallel walker using ignore::WalkBuilder.
///
/// ignore::WalkBuilder::build_parallel() uses a work-stealing thread pool
/// internally (same as jwalk), so there is no throughput regression from
/// dropping jwalk. The benefit is one code path, one dependency, and no
/// runtime branch on `config.gitignore`.
///
/// Workers write matches directly to thread-local BufWriters — no channel,
/// no shared queue, no per-match lock acquisition.
fn walk_parallel(
    root: &Path,
    config: Arc<WalkConfig>,
    slots: Arc<OutputSlots>,
    totals: Totals,
) {
    let mut builder = WalkBuilder::new(root);
    builder
        .follow_links(false)
        .hidden(false)
        .parents(false)       // don't read .ignore/.gitignore from parent dirs
        .ignore(false)        // don't read .ignore files
        .git_ignore(config.gitignore)
        .git_global(config.gitignore)
        .git_exclude(config.gitignore);

    if let Some(depth) = config.max_depth {
        builder.max_depth(Some(depth));
    }

    let walker = builder.build_parallel();

    walker.run(|| {
        let mut state = WorkerState::new(
            Arc::clone(&config),
            Arc::clone(&slots),
            Arc::clone(&totals),
        );

        Box::new(move |entry| {
            use ignore::WalkState;
            match entry {
                Ok(e) => {
                    let path = e.path();

                    // file_type() is free: ignore reads d_type from readdir,
                    // no extra stat syscall on Linux/macOS.
                    let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);

                    // Skip excluded directories before recursing.
                    // NOTE: with follow_links(false), file_type() reports the
                    // symlink itself — not its target — so a symlink pointing
                    // to an excluded directory will NOT be skipped here; it
                    // will be visited as a file entry instead. This is the
                    // correct, safe behavior: we never follow symlinks, so
                    // there is no risk of recursing into the excluded tree.
                    if is_dir && should_skip_dir(path, &state.config.exclude) {
                        if state.config.verbose {
                            let _ = eprintln!("[SKIP] {}", path.display());
                        }
                        return WalkState::Skip;
                    }

                    process_entry(path, is_dir, &mut state);
                    WalkState::Continue
                }
                Err(e) => {
                    if state.config.verbose {
                        let _ = eprintln!("[ERROR] {}", e);
                    }
                    WalkState::Continue
                }
            }
        })
    });
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let cli = Cli::parse();

    let mode = if cli.substr {
        MatchMode::Substr
    } else if cli.precise {
        MatchMode::Precise
    } else {
        MatchMode::Standard
    };

    let entry_type = match cli.entry_type.as_deref() {
        Some("f") => EntryType::File,
        Some("d") => EntryType::Dir,
        None | Some("a") => EntryType::Any,
        Some(other) => {
            eprintln!("fafind: unknown --type '{}' (use f or d)", other);
            std::process::exit(2);
        }
    };

    let root = cli.root.unwrap_or_else(|| PathBuf::from("/"));

    // Build the exclude list as a SmallVec of owned byte slices.
    // Typically 0–8 entries; lives entirely on the stack.
    let exclude: ExcludeList = cli
        .exclude
        .iter()
        .map(|s| s.as_bytes().to_vec().into_boxed_slice())
        .collect();

    let config = Arc::new(WalkConfig {
        target: MatchTarget::new(&cli.target, mode, cli.ignore_case),
        max_depth: cli.max_depth,
        exclude: Arc::new(exclude),
        entry_type,
        null_terminate: cli.null,
        gitignore: cli.gitignore,
        verbose: cli.verbose,
    });

    let start = Instant::now();
    let totals: Totals = Arc::new(Mutex::new((0u64, 0u64)));

    // Pre-allocate one output slot per worker thread.
    // ignore::WalkBuilder uses num_cpus by default; mirror that here.
    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let slots = OutputSlots::new(num_threads + 2); // +2: small headroom

    walk_parallel(&root, Arc::clone(&config), Arc::clone(&slots), Arc::clone(&totals));

    // All workers have exited (walk_parallel blocks until completion).
    // Drain slots in index order and write to stdout — no locking, no sync.
    {
        let stdout = std::io::stdout();
        let mut out = BufWriter::with_capacity(WRITER_BUF_CAP, stdout.lock());
        for buf in slots.drain_ordered() {
            let _ = out.write_all(&buf);
        }
    }

    let elapsed = start.elapsed();
    let secs = elapsed.as_secs_f64();
    let (scanned, found) = *totals.lock().unwrap();

    let files_per_sec = if secs > 0.0 {
        scanned as f64 / secs
    } else {
        f64::INFINITY
    };

    if !cli.quiet {
        eprintln!(
            "fafind: scanned {} files in {:.2}s ({:.0} files/sec), found {} matches",
            scanned,
            secs,
            files_per_sec,
            found
        );
    }

    // Exit codes follow grep convention:
    //   0 = at least one match found
    //   1 = no matches (not an error — lets callers use $? in scripts)
    //   2 = usage / I/O error (emitted earlier via process::exit(2))
    std::process::exit(if found > 0 { 0 } else { 1 });
}
