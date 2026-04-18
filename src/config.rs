use smallvec::SmallVec;
use std::sync::Arc;

use crate::matcher::MatchTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchMode {
    Substr,
    Precise,
    Standard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryType {
    Any,
    File,
    Dir,
}

// Exclude lists are typically 0–8 entries. SmallVec<[_; 8]> keeps them on
// the stack, avoiding a heap allocation AND replacing the hash-table lookup
// with a linear scan that is faster for n < ~16 due to cache locality.
pub type ExcludeList = SmallVec<[Box<[u8]>; 8]>;

pub struct WalkConfig {
    pub target: MatchTarget,
    pub max_depth: Option<usize>,
    /// Excluded directory names as raw byte slices for zero-copy linear scan.
    pub exclude: Arc<ExcludeList>,
    pub entry_type: EntryType,
    pub null_terminate: bool,
    pub gitignore: bool,
    pub verbose: bool,
}
