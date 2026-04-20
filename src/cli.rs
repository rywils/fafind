use clap::{Parser, ValueEnum};
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
#[command(version)]
#[command(about = "Fast filesystem search by filename")]
#[command(after_help = AFTER_HELP)]
#[command(arg_required_else_help = true)]
pub struct Cli {
    #[arg(short = 's', long)]
    pub substr: bool,

    #[arg(short = 'p', long)]
    pub precise: bool,

    /// Print every scanned file
    #[arg(short = 'v', long)]
    pub verbose: bool,

    /// Case-insensitive matching
    #[arg(short = 'i', long)]
    pub ignore_case: bool,

    /// Filter by entry type: f = files only, d = directories only
    #[arg(long = "type")]
    pub entry_type: Option<String>,

    /// Separate output with NUL instead of newline (for xargs -0)
    #[arg(short = '0', long = "null")]
    pub null: bool,

    #[arg(long)]
    pub max_depth: Option<usize>,

    #[arg(long, value_delimiter = ',')]
    pub exclude: Vec<String>,

    /// Respect .gitignore files
    #[arg(long)]
    pub gitignore: bool,

    /// Suppress the summary line (scanned/found/elapsed)
    #[arg(short = 'q', long)]
    pub quiet: bool,

    #[arg(long, value_enum, default_value = "auto")]
    pub color: ColorMode,

    #[arg(value_name = "TARGET")]
    pub target: String,

    #[arg(value_name = "ROOT")]
    pub root: Option<PathBuf>,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}
