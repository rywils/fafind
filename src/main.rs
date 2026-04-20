mod cli;
mod config;
mod matcher;
mod output;
mod util;
mod walker;
mod worker;

use clap::Parser;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use cli::{Cli, ColorMode};
use config::{EntryType, ExcludeList, MatchMode, WalkConfig};
use matcher::MatchTarget;
use output::OutputSlots;
use walker::walk_parallel;
use worker::Totals;

const WRITER_BUF_CAP: usize = 256 * 1024;

fn main() {
    let cli = Cli::parse();

    if cli.substr && cli.precise {
        eprintln!("error: cannot use -s and -p together");
        std::process::exit(2);
    }

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

    let exclude: ExcludeList = cli
        .exclude
        .iter()
        .map(|s| s.as_bytes().to_vec().into_boxed_slice())
        .collect();

    let stdout_is_tty = atty::is(atty::Stream::Stdout);
    let color = !cli.null
        && match cli.color {
            ColorMode::Never => false,
            ColorMode::Always => true,
            ColorMode::Auto => stdout_is_tty,
        };

    let target_raw: Arc<str> = cli.target.clone().into();
    let target_canonical: Arc<[u8]> = if cli.ignore_case {
        cli.target.to_ascii_lowercase().into_bytes().into()
    } else {
        cli.target.as_bytes().to_vec().into_boxed_slice().into()
    };

    let config = Arc::new(WalkConfig {
        target: MatchTarget::new(&cli.target, mode, cli.ignore_case),
        target_raw,
        target_canonical,
        match_mode: mode,
        ignore_case: cli.ignore_case,
        max_depth: cli.max_depth,
        exclude: Arc::new(exclude),
        entry_type,
        null_terminate: cli.null,
        gitignore: cli.gitignore,
        verbose: cli.verbose,
        color,
    });

    let start = Instant::now();
    let totals: Totals = Arc::new(Mutex::new((0u64, 0u64)));

    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let slots = OutputSlots::new(num_threads + 2); // +2: small headroom

    walk_parallel(&root, Arc::clone(&config), Arc::clone(&slots), Arc::clone(&totals));

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
