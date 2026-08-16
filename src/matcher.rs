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
        let effective = if mode == MatchMode::Standard {
            std::str::from_utf8(stem_bytes(raw.as_bytes())).unwrap_or(raw)
        } else {
            raw
        };

        let canonical: Arc<[u8]> = if ignore_case {
            effective.to_ascii_lowercase().into_bytes().into()
        } else {
            effective.as_bytes().into()
        };
        let target_is_ascii = effective.is_ascii();
        let canonical_len = canonical.len();
        let short_needle = canonical_len <= SHORT_NEEDLE_THRESHOLD;
        Self {
            canonical,
            canonical_len,
            mode,
            ignore_case,
            target_is_ascii,
            short_needle,
        }
    }

    /// The case/stem-normalized query bytes used for matching.
    #[inline(always)]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }

    /// True if the query is ASCII-only (cached at construction).
    #[inline(always)]
    pub fn is_ascii_target(&self) -> bool {
        self.target_is_ascii
    }

    /// Hot path returns true if `filename` matches this target.
    #[inline(always)]
    pub fn is_match(&self, bytes: &[u8]) -> bool {
        match self.mode {
            MatchMode::Precise => {
                if bytes.len() != self.canonical_len {
                    return false;
                }
                self.match_precise(bytes)
            }
            MatchMode::Substr => {
                if bytes.len() < self.canonical_len {
                    return false;
                }
                self.match_substr(bytes)
            }
            MatchMode::Standard => {
                let stem = stem_bytes(bytes);
                if stem.len() != self.canonical_len {
                    return false;
                }
                self.match_standard(stem)
            }
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

    /// `stem` is the filename stem already extracted by the caller
    /// (`is_match`) - the length check against `canonical_len` already
    /// happened there, so this only does the byte comparison.
    #[inline(always)]
    fn match_standard(&self, stem: &[u8]) -> bool {
        if !self.ignore_case {
            return stem == self.canonical.as_ref();
        }
        if self.target_is_ascii {
            ascii_eq_ignore_case_single_pass(stem, &self.canonical)
        } else {
            unicode_eq_ignore_case(stem, &self.canonical)
        }
    }
}

// Case-folding helpers
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
                if haystack[i].to_ascii_lowercase() == n0 {
                    return true;
                }
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
    let Ok(s) = std::str::from_utf8(bytes) else {
        return false;
    };
    s.to_lowercase().as_bytes() == canonical_lower
}

/// Unicode case-insensitive substring search (cold path).
#[cold]
fn unicode_contains_ignore_case(bytes: &[u8], needle_lower: &[u8]) -> bool {
    let Ok(s) = std::str::from_utf8(bytes) else {
        return false;
    };
    let lower = s.to_lowercase();
    memchr::memmem::find(lower.as_bytes(), needle_lower).is_some()
}

/// Extract the file stem as a byte slice from raw filename bytes.
#[inline(always)]
pub fn stem_bytes(bytes: &[u8]) -> &[u8] {
    let n = bytes.len();
    if n == 0 {
        return bytes;
    }
    let mut i = n;
    while i > 1 {
        // stop at 1: dot at index 0 = hidden file, stem = whole name
        i -= 1;
        if bytes[i] == b'.' {
            return &bytes[..i];
        }
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(mode: MatchMode, target: &str, filename: &str) -> bool {
        MatchTarget::new(target, mode, false).is_match(filename.as_bytes())
    }

    #[test]
    fn standard_stem_query_with_extension_matches_stem() {
        assert!(matches(MatchMode::Standard, "walker.rs", "walker.rs"));
        assert!(matches(MatchMode::Standard, "walker.rs", "walker.go"));
        assert!(!matches(MatchMode::Standard, "walker.rs", "mywalker.rs"));
    }

    #[test]
    fn standard_stem_query_without_extension_same_as_with() {
        assert_eq!(
            matches(MatchMode::Standard, "walker", "walker.rs"),
            matches(MatchMode::Standard, "walker.rs", "walker.rs"),
        );
    }

    #[test]
    fn precise_requires_full_filename() {
        assert!(matches(MatchMode::Precise, "walker.rs", "walker.rs"));
        assert!(!matches(MatchMode::Precise, "walker", "walker.rs"));
        assert!(!matches(MatchMode::Precise, "walker.rs", "walker.go"));
    }
}
