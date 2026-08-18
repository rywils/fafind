use std::path::Path;
use std::sync::{Arc, Mutex};

use ignore::WalkBuilder;

use crate::config::{ExcludeList, WalkConfig};
use crate::worker::{Totals, WorkerState, process_entry};

/// Returns true if this directory should be skipped.
/// Linear scan over SmallVec — faster than HashSet for n ≤ ~16.
#[inline(always)]
pub fn should_skip_dir(path: &Path, exclude: &ExcludeList) -> bool {
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

/// Single unified parallel walker using ignore::WalkBuilder.
/// ignore::WalkBuilder::build_parallel() uses a work-stealing thread pool.
/// Workers stream matches to stdout as they are found.
pub fn walk_parallel(
    root: &Path,
    config: Arc<WalkConfig>,
    totals: Arc<Totals>,
    cache_sink: Option<Arc<Mutex<Vec<u8>>>>,
) {
    let mut builder = WalkBuilder::new(root);
    builder
        .follow_links(false)
        .hidden(false)
        .parents(false) // don't read .ignore/.gitignore from parent dirs
        .ignore(false) // don't read .ignore files
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
            Arc::clone(&totals),
            cache_sink.clone(),
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
                    if is_dir && should_skip_dir(path, &state.config.exclude) {
                        if state.verbose {
                            eprintln!("[SKIP] {}", path.display());
                        }
                        return WalkState::Skip;
                    }

                    process_entry(path, is_dir, e.depth() == 0, &mut state);
                    WalkState::Continue
                }
                Err(e) => {
                    if state.verbose {
                        eprintln!("[ERROR] {}", e);
                    }
                    WalkState::Continue
                }
            }
        })
    });
}
