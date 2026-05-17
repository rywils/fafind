use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::config::{EntryType, WalkConfig};
use crate::util::{append_path, append_path_highlight};

pub const WORKER_BUF_CAP: usize = 256 * 1024; // 256 KB initial capacity per worker

pub type Totals = Arc<Mutex<(u64, u64)>>; // (scanned, found)

/// Worker state: entirely private per-thread.
/// Match lines are formatted in `out_buf` without locking; stdout is locked only to write.
pub struct WorkerState {
    pub config: Arc<WalkConfig>,
    pub local_scanned: u64,
    pub local_found: u64,
    pub out_buf: Vec<u8>,
    pub totals: Totals,
}

impl WorkerState {
    pub fn new(config: Arc<WalkConfig>, totals: Totals) -> Self {
        Self {
            config,
            local_scanned: 0,
            local_found: 0,
            out_buf: Vec::with_capacity(WORKER_BUF_CAP),
            totals,
        }
    }
}

impl Drop for WorkerState {
    fn drop(&mut self) {
        flush_out_buf(&mut self.out_buf, &self.config);
        if let Ok(mut t) = self.totals.lock() {
            t.0 += self.local_scanned;
            t.1 += self.local_found;
        }
    }
}

/// Write pending match bytes to stdout. Formatting stays lock-free; lock covers one write only.
#[inline(always)]
fn flush_out_buf(out_buf: &mut Vec<u8>, config: &WalkConfig) {
    if out_buf.is_empty() {
        return;
    }
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(out_buf);
    if config.stdout_block_buffered {
        let _ = stdout.flush();
    }
    out_buf.clear();
}

/// Hot path: called for every filesystem entry.
/// Zero syscalls, zero heap allocations, zero shared-memory writes (except match flush).
#[inline(always)]
pub fn process_entry(path: &Path, is_dir: bool, state: &mut WorkerState) {
    state.local_scanned += 1;

    match state.config.entry_type {
        EntryType::File if is_dir => return,
        EntryType::Dir if !is_dir => return,
        _ => {}
    }

    let Some(filename) = path.file_name() else { return };

    let cfg = &*state.config;

    if cfg.verbose {
        verbose_scan(path);
    }

    if cfg.target.is_match(filename) {
        state.local_found += 1;
        if cfg.verbose {
            let s = format!("[MATCH] {}\n", path.display());
            state.out_buf.extend_from_slice(s.as_bytes());
        } else if cfg.color {
            append_path_highlight(&mut state.out_buf, path, cfg);
        } else {
            append_path(&mut state.out_buf, path, cfg.null_terminate);
        }
        flush_out_buf(&mut state.out_buf, cfg);
    }
}

/// Verbose scan log — cold path, out-of-line to keep process_entry tight.
#[cold]
#[inline(never)]
pub fn verbose_scan(path: &Path) {
    let _ = eprintln!("[SCAN] {}", path.display());
}
