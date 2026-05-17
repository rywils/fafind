mod cli;
mod config;
mod matcher;
mod util;
mod walker;
mod worker;

use clap::Parser;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use cli::{Cli, ColorMode};
use config::{EntryType, ExcludeList, MatchMode, WalkConfig};
use matcher::MatchTarget;
use walker::walk_parallel;
use worker::Totals;

#[inline(always)]
fn program_label() -> &'static str {
    std::env::args_os()
        .next()
        .as_ref()
        .and_then(|p| Path::new(p).file_name())
        .and_then(|s| s.to_str())
        .map(|s| if s == "faf" { "faf" } else { "fafind" })
        .unwrap_or("fafind")
}

pub fn run() {
    let cli = Cli::parse();
    let prog = program_label();

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
            eprintln!("{prog}: unknown --type '{}' (use f or d)", other);
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
        stdout_block_buffered: !stdout_is_tty,
    });

    let start = Instant::now();
    let totals = Totals::new();

    walk_parallel(&root, Arc::clone(&config), Arc::clone(&totals));

    let elapsed = start.elapsed();
    let secs = elapsed.as_secs_f64();
    let (scanned, found) = totals.snapshot();

    let files_per_sec = if secs > 0.0 {
        scanned as f64 / secs
    } else {
        f64::INFINITY
    };

    if !cli.quiet {
        eprintln!(
            "{prog}: scanned {} files in {:.2}s ({:.0} files/sec), found {} matches",
            scanned,
            secs,
            files_per_sec,
            found
        );
    }

    std::process::exit(if found > 0 { 0 } else { 1 });
}
