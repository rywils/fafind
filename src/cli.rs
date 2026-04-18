use clap::{ArgGroup, Parser};
use std::path::PathBuf;

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

#[derive(Parser, Debug)]
#[command(name = "fafind")]
#[command(version = "1.0.0")]
#[command(about = "Fast filesystem search by filename")]
#[command(after_help = AFTER_HELP)]
#[command(group(ArgGroup::new("mode").args(["substr", "precise"]).multiple(false)))]
pub struct Cli {
    pub target: String,

    #[arg(value_name = "ROOT")]
    pub root: Option<PathBuf>,

    /// Substring match: find files whose name contains TARGET anywhere
    #[arg(short = 's', long, group = "mode")]
    pub substr: bool,

    /// Exact match: full filename must equal TARGET (including extension)
    #[arg(short = 'p', long, group = "mode")]
    pub precise: bool,

    /// Print every scanned file
    #[arg(short = 'v', long)]
    pub verbose: bool,

    /// Case-insensitive matching
    #[arg(short = 'i', long)]
    pub ignore_case: bool,

    /// Filter by entry type: f = files only, d = directories only
    #[arg(long = "type", value_name = "f|d")]
    pub entry_type: Option<String>,

    /// Separate output with NUL instead of newline (for xargs -0)
    #[arg(short = '0', long = "null")]
    pub null: bool,

    /// Maximum directory depth to recurse into
    #[arg(long, value_name = "N")]
    pub max_depth: Option<usize>,

    /// Comma-separated list of directory names to exclude
    #[arg(long, value_name = "DIRS", value_delimiter = ',')]
    pub exclude: Vec<String>,

    /// Respect .gitignore files
    #[arg(long)]
    pub gitignore: bool,

    /// Suppress the summary line (scanned/found/elapsed)
    #[arg(short = 'q', long)]
    pub quiet: bool,
}
