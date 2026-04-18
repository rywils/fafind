use std::ffi::OsStr;
use std::sync::Arc;

use crate::config::MatchMode;

// Stack buffer constants
// FILENAME_BUF_LEN: stack buffer size for ASCII case-folding in substr mode.
// POSIX NAME_MAX = 255 on Linux/macOS/BSDs; +1 for headroom.
// Windows MAX_PATH component: 255 UTF-16 chars * up to 3 UTF-8 bytes = 765.
#[cfg(unix)]
const FILENAME_BUF_LEN: usize = 256; // NAME_MAX (255) + 1
#[cfg(not(unix))]
const FILENAME_BUF_LEN: usize = 768;

// Needle length threshold 
const SHORT_NEEDLE_THRESHOLD: usize = 4;

// MatchTarget
/// Holds the pre-processed target for matching.
/// Constructed once; shared read-only across all worker threads
#[derive(Clone)]
pub struct MatchTarget {
    canonical: Arc<[u8]>,
    /// Length of canonical, cached to avoid a pointer deref in the hot loop.
    canonical_len: usize,
    mode: MatchMode,
    ignore_case: bool,
    /// True if the target contains only ASCII bytes.
    target_is_ascii: bool,
    /// True if the needle is short enough (≤ 4 bytes) for the sliding-window
    short_needle: bool,
}

impl MatchTarget {
    pub fn new(raw: &str, mode: MatchMode, ignore_case: bool) -> Self {
        let canonical: Arc<[u8]> = if ignore_case {
            raw.to_ascii_lowercase().into_bytes().into()
        } else {
            raw.as_bytes().into()
        };
        let target_is_ascii = raw.is_ascii();
        let canonical_len = canonical.len();
        let short_needle = canonical_len <= SHORT_NEEDLE_THRESHOLD;
        Self { canonical, canonical_len, mode, ignore_case, target_is_ascii, short_needle }
    }

    /// Hot path returns true if `filename` matches this target.
   
    /// ASCII fast-path (covers >95% of real-world filenames):
    ///   - Detects ASCII-only filenames with a SIMD-friendly all-< 128 check.
    ///   - Performs case folding inline with `to_ascii_lowercase()` on a
    ///     stack-allocated copy — zero heap allocation.
   
    /// Unicode fallback:
    ///   - Only triggered when a byte ≥ 128 is present in the filename.
    ///   - Falls back to `to_lowercase()` which may allocate, but this is the
    ///     rare case
    #[inline(always)]
    pub fn is_match(&self, filename: &OsStr) -> bool {
        let bytes = filename.as_encoded_bytes();

        match self.mode {
            MatchMode::Precise => self.match_precise(bytes),
            MatchMode::Substr => self.match_substr(bytes),
            MatchMode::Standard => self.match_standard(bytes),
        }
    }

    #[inline(always)]
    fn match_precise(&self, bytes: &[u8]) -> bool {
        if !self.ignore_case {
            return bytes == self.canonical.as_ref();
        }
        if bytes.len() != self.canonical_len {
            return false;
        }
        if self.target_is_ascii {
            ascii_eq_ignore_case_single_pass(bytes, &self.canonical)
        } else {
            unicode_eq_ignore_case(bytes, &self.canonical)
        }
    }

    #[inline(always)]
    fn match_substr(&self, bytes: &[u8]) -> bool {
        if !self.ignore_case {
            // Case-sensitive: SIMD memmem, zero allocation.
            return memchr::memmem::find(bytes, &self.canonical).is_some();
        }
        if self.target_is_ascii {
            // ASCII/Unicode decision made ONCE here
            // bytes.is_ascii() is a tight all-< 128 scan the compiler
            // auto-vectorizes; it runs once and gates the entire hot path.
            if bytes.is_ascii() {
                if self.short_needle {
                    ascii_substr_short(bytes, &self.canonical)
                } else {
                    ascii_contains_ignore_case(bytes, &self.canonical)
                }
            } else {
                unicode_contains_ignore_case(bytes, &self.canonical)
            }
        } else {
            unicode_contains_ignore_case(bytes, &self.canonical)
        }
    }

    #[inline(always)]
    fn match_standard(&self, bytes: &[u8]) -> bool {
        let stem = stem_bytes(bytes);
        if !self.ignore_case {
            return stem == self.canonical.as_ref();
        }
        if stem.len() != self.canonical_len {
            return false;
        }
        if self.target_is_ascii {
            ascii_eq_ignore_case_single_pass(stem, &self.canonical)
        } else {
            unicode_eq_ignore_case(stem, &self.canonical)
        }
    }
}

// Case-folding helpers
// Single-pass ASCII case-insensitive equality.
// One loop. Non-ASCII detection + lowercased compare simultaneously.
// No separate is_ascii() pre-scan.
#[inline(always)]
fn ascii_eq_ignore_case_single_pass(bytes: &[u8], canonical: &[u8]) -> bool {
    let n = bytes.len(); 
    let mut i = 0usize;
    while i < n {
        let a = bytes[i];
        if a >= 128 {
            return unicode_eq_ignore_case(bytes, canonical);
        }
        if a.to_ascii_lowercase() != canonical[i] {
            return false;
        }
        i += 1;
    }
    true
}

// Short-needle ASCII case-insensitive substring search.
// PRECONDITION: caller has confirmed haystack.is_ascii() == true.
// No >= 128 checks inside — the hot loops are branch-free straight-line
// compares: only the window-advance test and one equality chain per position.

#[inline(always)]
fn ascii_substr_short(haystack: &[u8], needle: &[u8]) -> bool {
    let hlen = haystack.len();
    let nlen = needle.len();
    if hlen < nlen {
        return false;
    }
    let limit = hlen - nlen;
    match nlen {
        1 => {
            let n0 = needle[0];
            let mut i = 0usize;
            while i <= limit {
                if haystack[i].to_ascii_lowercase() == n0 { return true; }
                i += 1;
            }
        }
        2 => {
            let (n0, n1) = (needle[0], needle[1]);
            let mut i = 0usize;
            while i <= limit {
                if haystack[i].to_ascii_lowercase() == n0
                    && haystack[i + 1].to_ascii_lowercase() == n1
                {
                    return true;
                }
                i += 1;
            }
        }
        3 => {
            let (n0, n1, n2) = (needle[0], needle[1], needle[2]);
            let mut i = 0usize;
            while i <= limit {
                if haystack[i].to_ascii_lowercase() == n0
                    && haystack[i + 1].to_ascii_lowercase() == n1
                    && haystack[i + 2].to_ascii_lowercase() == n2
                {
                    return true;
                }
                i += 1;
            }
        }
        4 => {
            let (n0, n1, n2, n3) = (needle[0], needle[1], needle[2], needle[3]);
            let mut i = 0usize;
            while i <= limit {
                if haystack[i].to_ascii_lowercase() == n0
                    && haystack[i + 1].to_ascii_lowercase() == n1
                    && haystack[i + 2].to_ascii_lowercase() == n2
                    && haystack[i + 3].to_ascii_lowercase() == n3
                {
                    return true;
                }
                i += 1;
            }
        }
        _ => return true, 
    }
    false
}

/// ASCII case-insensitive substring search using a stack buffer + memmem.
/// PRECONDITION: caller has confirmed haystack.is_ascii() == true.
/// No >= 128 guard inside the fill loop — pure lowercase + copy,
/// no branches other than the loop counter. memmem then runs SIMD over
/// the contiguous lowercased buffer.
#[inline(always)]
fn ascii_contains_ignore_case(haystack: &[u8], needle: &[u8]) -> bool {
    if haystack.len() <= FILENAME_BUF_LEN {
        let mut buf = [0u8; FILENAME_BUF_LEN];
        let h = &mut buf[..haystack.len()];
        for (dst, &src) in h.iter_mut().zip(haystack.iter()) {
            *dst = src.to_ascii_lowercase();
        }
        memchr::memmem::find(h, needle).is_some()
    } else {
        // Practically unreachable on any filesystem (exceeds NAME_MAX).
        let lower: Vec<u8> = haystack.iter().map(|b| b.to_ascii_lowercase()).collect();
        memchr::memmem::find(&lower, needle).is_some()
    }
}

/// Unicode case-insensitive equality check (cold path).
#[cold]
fn unicode_eq_ignore_case(bytes: &[u8], canonical_lower: &[u8]) -> bool {
    let Ok(s) = std::str::from_utf8(bytes) else { return false };
    s.to_lowercase().as_bytes() == canonical_lower
}

/// Unicode case-insensitive substring search (cold path).
#[cold]
fn unicode_contains_ignore_case(bytes: &[u8], needle_lower: &[u8]) -> bool {
    let Ok(s) = std::str::from_utf8(bytes) else { return false };
    let lower = s.to_lowercase();
    memchr::memmem::find(lower.as_bytes(), needle_lower).is_some()
}

/// Extract the file stem as a byte slice from raw filename bytes.
/// Equivalent to Path::file_stem() but zero-allocation.
/// Manual reverse scan replaces iterator state machine from rposition.
#[inline(always)]
pub fn stem_bytes(bytes: &[u8]) -> &[u8] {
    let n = bytes.len();
    if n == 0 { return bytes; }
    let mut i = n;
    while i > 1 { // stop at 1: dot at index 0 = hidden file, stem = whole name
        i -= 1;
        if bytes[i] == b'.' {
            return &bytes[..i];
        }
    }
    bytes
}
