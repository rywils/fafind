use std::path::Path;

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

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
