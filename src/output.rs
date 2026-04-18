use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

// Lock-free output slot array.
// Layout: (next_slot_idx, [UnsafeCell<Option<Vec<u8>>>; num_workers])

pub struct OutputSlots {
    next_idx: AtomicUsize,
    slots: Vec<UnsafeCell<Option<Vec<u8>>>>,
}

// SAFETY: Workers only write to their own unique slot; main reads only after
// all workers are done. Unique ownership per slot guarantees no data races.
unsafe impl Sync for OutputSlots {}
unsafe impl Send for OutputSlots {}

impl OutputSlots {
    pub fn new(capacity: usize) -> Arc<Self> {
        let mut slots = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            slots.push(UnsafeCell::new(None));
        }
        Arc::new(Self { next_idx: AtomicUsize::new(0), slots })
    }

    // Claim the next available slot index. Called once per worker at construction.
    pub fn claim(&self) -> usize {
        self.next_idx.fetch_add(1, Ordering::Relaxed)
    }

    // Write buf into slot. Caller must hold the unique index for this slot.
    pub unsafe fn write(&self, idx: usize, buf: Vec<u8>) {
        if idx < self.slots.len() {
            unsafe { *self.slots[idx].get() = Some(buf); }
        }
    }

    // Drain all slots in index order. Called by main after all workers have exited.
    pub fn drain_ordered(self: Arc<Self>) -> impl Iterator<Item = Vec<u8>> {
        // Unwrap the Arc — main is the only holder at this point.
        let inner = Arc::try_unwrap(self)
            .unwrap_or_else(|_| panic!("OutputSlots Arc still shared after walk"));
        inner.slots.into_iter().filter_map(|cell| cell.into_inner())
    }
}
