use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::config::{EntryType, WalkConfig};
use crate::output::OutputSlots;
use crate::util::append_path;

pub const WORKER_BUF_CAP: usize = 256 * 1024; // 256 KB initial capacity per worker

pub type Totals = Arc<Mutex<(u64, u64)>>; // (scanned, found)

/// Worker state: entirely private per-thread.
/// Zero shared-memory writes in the hot path.
pub struct WorkerState {
    pub config: Arc<WalkConfig>,
    pub local_scanned: u64,
    pub local_found: u64,
    /// Accumulated match output. Grown inline; written to slot on drop.
    pub out_buf: Vec<u8>,
    /// Lock-free output slot array + this worker's unique slot index.
    pub slots: Arc<OutputSlots>,
    pub slot_idx: usize,
    pub totals: Totals,
}

impl WorkerState {
    pub fn new(config: Arc<WalkConfig>, slots: Arc<OutputSlots>, totals: Totals) -> Self {
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
            // slot_idx is unique to this worker; no other thread writes here.
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
        } else {
            append_path(&mut state.out_buf, path, cfg.null_terminate);
        }
    }
}

/// Verbose scan log — cold path, out-of-line to keep process_entry tight.
#[cold]
#[inline(never)]
pub fn verbose_scan(path: &Path) {
    let _ = eprintln!("[SCAN] {}", path.display());
}
