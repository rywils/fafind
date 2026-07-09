use std::path::Path;

use crate::config::{MatchMode, WalkConfig};
use crate::matcher::stem_bytes;

const GREEN: &[u8] = b"\x1b[32m";
const DIM: &[u8] = b"\x1b[2m";
const BOLD: &[u8] = b"\x1b[1m";
const YELLOW: &[u8] = b"\x1b[33m";
/// Non-match filename segments in substring mode (`-s`).
const ORANGE: &[u8] = b"\x1b[38;5;208m";
const RESET: &[u8] = b"\x1b[0m";

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

/// Fast filename extraction for non-root walk entries.
///
/// Bypasses `Path::file_name()` (which builds a `Components` iterator to
/// handle `.`/`..`/prefix/trailing-slash normalization) with a raw reverse
/// byte scan. Safe here because entry names come straight from `readdir`
/// and are never `.`, `..`, or slash-terminated - that generality is only
/// needed for the walk root itself, which callers must handle separately.
#[cfg(unix)]
#[inline(always)]
pub fn entry_file_name_bytes(path: &Path) -> &[u8] {
    let full = path.as_os_str().as_bytes();
    match memchr::memrchr(b'/', full) {
        Some(i) => &full[i + 1..],
        None => full,
    }
}

/// Append a matched path to a raw byte buffer.
///
/// On Unix: appends the raw OS bytes via OsStrExt::as_bytes() — no UTF-8
/// validation, no format string, just a slice extend.
/// On non-Unix: falls back to path.display().
#[inline(always)]
pub fn append_path(buf: &mut Vec<u8>, path: &Path, null_terminate: bool) {
    #[cfg(unix)]
    {
        buf.extend_from_slice(path.as_os_str().as_bytes());
        if null_terminate { buf.push(b'\0'); } else { buf.push(b'\n'); }
    }
    #[cfg(not(unix))]
    {
        let s = if null_terminate {
            format!("{}\0", path.display())
        } else {
            format!("{}\n", path.display())
        };
        buf.extend_from_slice(s.as_bytes());
    }
}

#[inline(always)]
pub fn append_path_highlight(buf: &mut Vec<u8>, path: &Path, cfg: &WalkConfig) {
    if cfg.null_terminate || !cfg.color {
        append_path(buf, path, cfg.null_terminate);
        return;
    }

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;

        let full = path.as_os_str().as_bytes();
        let slash = full.iter().rposition(|&b| b == b'/');
        let (prefix, name_bytes) = match slash {
            Some(i) => (&full[..=i], &full[i + 1..]),
            None => (&[][..], full),
        };

        if !prefix.is_empty() {
            buf.extend_from_slice(DIM);
            buf.extend_from_slice(prefix);
            buf.extend_from_slice(RESET);
        }

        match cfg.match_mode {
            MatchMode::Precise => {
                let stem_len = stem_bytes(name_bytes).len();
                buf.extend_from_slice(BOLD);
                buf.extend_from_slice(GREEN);
                buf.extend_from_slice(&name_bytes[..stem_len]);
                buf.extend_from_slice(RESET);
                let ext = &name_bytes[stem_len..];
                if !ext.is_empty() {
                    buf.extend_from_slice(YELLOW);
                    buf.extend_from_slice(ext);
                    buf.extend_from_slice(RESET);
                }
            }
            MatchMode::Standard => {
                let stem_len = stem_bytes(name_bytes).len();
                buf.extend_from_slice(GREEN);
                buf.extend_from_slice(&name_bytes[..stem_len]);
                buf.extend_from_slice(RESET);

                let ext = &name_bytes[stem_len..];
                if !ext.is_empty() {
                    buf.extend_from_slice(YELLOW);
                    buf.extend_from_slice(ext);
                    buf.extend_from_slice(RESET);
                }
            }
            MatchMode::Substr => {
                let stem_len = stem_bytes(name_bytes).len();
                let ext = &name_bytes[stem_len..];

                if cfg.ignore_case {
                    if !name_bytes.is_ascii() || !cfg.target_canonical.is_ascii() {
                        buf.extend_from_slice(name_bytes);
                        buf.push(b'\n');
                        return;
                    }
                    highlight_substr_ascii_ignore_case(buf, &name_bytes[..stem_len], &cfg.target_canonical);
                } else {
                    let needle = cfg.target_raw.as_bytes();
                    highlight_substr_bytes(buf, &name_bytes[..stem_len], needle);
                }

                if !ext.is_empty() {
                    buf.extend_from_slice(YELLOW);
                    buf.extend_from_slice(ext);
                    buf.extend_from_slice(RESET);
                }
            }
        }

        buf.push(b'\n');
        return;
    }

    #[cfg(not(unix))]
    {
        append_path(buf, path, false);
    }
}

#[inline(always)]
fn highlight_substr_bytes(buf: &mut Vec<u8>, name: &[u8], needle: &[u8]) {
    if needle.is_empty() {
        buf.extend_from_slice(ORANGE);
        buf.extend_from_slice(name);
        buf.extend_from_slice(RESET);
        return;
    }

    let mut i = 0usize;
    while i <= name.len() {
        let Some(rel) = memchr::memmem::find(&name[i..], needle) else {
            buf.extend_from_slice(ORANGE);
            buf.extend_from_slice(&name[i..]);
            buf.extend_from_slice(RESET);
            return;
        };
        let at = i + rel;
        buf.extend_from_slice(ORANGE);
        buf.extend_from_slice(&name[i..at]);
        buf.extend_from_slice(RESET);
        buf.extend_from_slice(GREEN);
        buf.extend_from_slice(&name[at..at + needle.len()]);
        buf.extend_from_slice(RESET);
        i = at + needle.len();
    }
}

#[inline(always)]
fn highlight_substr_ascii_ignore_case(buf: &mut Vec<u8>, name: &[u8], needle_lower: &[u8]) {
    if needle_lower.is_empty() {
        buf.extend_from_slice(ORANGE);
        buf.extend_from_slice(name);
        buf.extend_from_slice(RESET);
        return;
    }

    let n = needle_lower.len();
    let mut i = 0usize;
    let mut last = 0usize;

    while i + n <= name.len() {
        let mut j = 0usize;
        while j < n {
            if name[i + j].to_ascii_lowercase() != needle_lower[j] {
                break;
            }
            j += 1;
        }

        if j == n {
            buf.extend_from_slice(ORANGE);
            buf.extend_from_slice(&name[last..i]);
            buf.extend_from_slice(RESET);
            buf.extend_from_slice(GREEN);
            buf.extend_from_slice(&name[i..i + n]);
            buf.extend_from_slice(RESET);
            i += n;
            last = i;
        } else {
            i += 1;
        }
    }

    buf.extend_from_slice(ORANGE);
    buf.extend_from_slice(&name[last..]);
    buf.extend_from_slice(RESET);
}
