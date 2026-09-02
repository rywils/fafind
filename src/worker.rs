use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::CacheWriter;
use crate::config::{EntryType, WalkConfig};
use crate::matcher::MatchTarget;
use crate::util::{append_path, append_path_highlight};

#[cfg(unix)]
use crate::util::entry_file_name_bytes;

pub const WORKER_BUF_CAP: usize = 256 * 1024; // 256 KB initial capacity per worker
const STREAM_BATCH_THRESHOLD: usize = 64 * 1024;

pub struct Totals {
    scanned: AtomicU64,
    found: AtomicU64,
}

impl Totals {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            scanned: AtomicU64::new(0),
            found: AtomicU64::new(0),
        })
    }

    pub fn snapshot(&self) -> (u64, u64) {
        (
            self.scanned.load(Ordering::Relaxed),
            self.found.load(Ordering::Relaxed),
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EntryFilter {
    Any,
    FileOnly,
    DirOnly,
}

/// Worker state
pub struct WorkerState {
    target: MatchTarget,
    entry_filter: EntryFilter,
    pub verbose: bool,
    color: bool,
    null_terminate: bool,
    stdout_block_buffered: bool,
    pub config: Arc<WalkConfig>,
    local_scanned: u64,
    local_found: u64,
    out_buf: Vec<u8>,
    totals: Arc<Totals>,
    cache_buf: Vec<u8>,
    cache_writer: Option<Arc<CacheWriter>>,
}

impl WorkerState {
    pub fn new(
        config: Arc<WalkConfig>,
        totals: Arc<Totals>,
        cache_writer: Option<Arc<CacheWriter>>,
    ) -> Self {
        let entry_filter = match config.entry_type {
            EntryType::File => EntryFilter::FileOnly,
            EntryType::Dir => EntryFilter::DirOnly,
            EntryType::Any => EntryFilter::Any,
        };
        Self {
            target: config.target.clone(),
            entry_filter,
            verbose: config.verbose,
            color: config.color,
            null_terminate: config.null_terminate,
            stdout_block_buffered: config.stdout_block_buffered,
            config,
            local_scanned: 0,
            local_found: 0,
            out_buf: Vec::with_capacity(WORKER_BUF_CAP),
            totals,
            cache_buf: Vec::new(),
            cache_writer,
        }
    }
}

impl Drop for WorkerState {
    fn drop(&mut self) {
        flush_out_buf(&mut self.out_buf, self.stdout_block_buffered);
        if let Some(writer) = &self.cache_writer
            && !self.cache_buf.is_empty()
        {
            writer.write(&self.cache_buf);
        }
        self.totals
            .scanned
            .fetch_add(self.local_scanned, Ordering::Relaxed);
        self.totals
            .found
            .fetch_add(self.local_found, Ordering::Relaxed);
    }
}

/// Write pending match bytes to stdout.
#[inline(always)]
fn flush_out_buf(out_buf: &mut Vec<u8>, stdout_block_buffered: bool) {
    if out_buf.is_empty() {
        return;
    }
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(out_buf);
    if stdout_block_buffered {
        let _ = stdout.flush();
    }
    out_buf.clear();
}

#[inline(always)]
fn maybe_flush_matches(out_buf: &mut Vec<u8>, stdout_block_buffered: bool) {
    if stdout_block_buffered || out_buf.len() >= STREAM_BATCH_THRESHOLD {
        flush_out_buf(out_buf, stdout_block_buffered);
    }
}

/// Hot path: called for every filesystem entry.
///
/// `is_root` marks the walk's starting entry (depth 0), whose path comes
/// from the user (may be `.`, `/`, or end in `/`) and needs the general
/// `Path::file_name()` parsing. Every other entry's name comes straight
/// from `readdir` and is never `.`, `..`, or slash-terminated, so it can
/// use the raw byte scan below instead of building a `Components` iterator.
#[inline(always)]
pub fn process_entry(path: &Path, is_dir: bool, is_root: bool, state: &mut WorkerState) {
    state.local_scanned += 1;

    match state.entry_filter {
        EntryFilter::FileOnly if is_dir => return,
        EntryFilter::DirOnly if !is_dir => return,
        EntryFilter::Any | EntryFilter::FileOnly | EntryFilter::DirOnly => {}
    }

    #[cfg(unix)]
    let filename: &[u8] = if is_root {
        let Some(f) = path.file_name() else { return };
        f.as_encoded_bytes()
    } else {
        let f = entry_file_name_bytes(path);
        if f.is_empty() {
            return;
        }
        f
    };
    #[cfg(not(unix))]
    let filename: &[u8] = {
        let Some(f) = path.file_name() else { return };
        f.as_encoded_bytes()
    };

    if state.verbose {
        verbose_scan(path);
    }

    if state.target.is_match(filename) {
        state.local_found += 1;
        if let Some(writer) = &state.cache_writer {
            append_path(&mut state.cache_buf, path, true);
            if state.cache_buf.len() >= STREAM_BATCH_THRESHOLD {
                writer.write(&state.cache_buf);
                state.cache_buf.clear();
            }
        }
        if state.verbose {
            let s = format!("[MATCH] {}\n", path.display());
            state.out_buf.extend_from_slice(s.as_bytes());
        } else if state.color {
            append_path_highlight(&mut state.out_buf, path, &state.config);
        } else {
            append_path(&mut state.out_buf, path, state.null_terminate);
        }
        maybe_flush_matches(&mut state.out_buf, state.stdout_block_buffered);
    }
}

/// Verbose scan log
#[cold]
#[inline(never)]
pub fn verbose_scan(path: &Path) {
    eprintln!("[SCAN] {}", path.display());
}
