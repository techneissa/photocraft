//! Fork: how much pixel memory a PSD import may estimate. Official code stops at
//! [`DEFAULT_PSD_IMPORT_PIXEL_BYTES`] (2 GiB), which refuses a 1 GB layered file whose estimate is
//! 3.6 GB. On a 64-bit Mac the allowance follows the installed memory instead: half of it, and
//! never below that default. A file past it still gets the same "needs an estimated N bytes" error.

use crate::DEFAULT_PSD_IMPORT_PIXEL_BYTES;

/// The allowance for a machine with `total` bytes of memory (`None`: not known).
pub fn limit_for(total: Option<u64>) -> u64 {
    if !cfg!(target_pointer_width = "64") || cfg!(target_arch = "wasm32") {
        return DEFAULT_PSD_IMPORT_PIXEL_BYTES;
    }
    total.map_or(DEFAULT_PSD_IMPORT_PIXEL_BYTES, |t| (t / 2).max(DEFAULT_PSD_IMPORT_PIXEL_BYTES))
}

/// The allowance on this machine; replaces the official `DEFAULT_PSD_IMPORT_PIXEL_BYTES` caps.
pub fn psd_import_pixel_bytes() -> u64 {
    limit_for(installed_memory())
}

#[cfg(target_os = "macos")]
fn installed_memory() -> Option<u64> {
    static TOTAL: std::sync::OnceLock<Option<u64>> = std::sync::OnceLock::new();
    *TOTAL.get_or_init(|| {
        let out = std::process::Command::new("/usr/sbin/sysctl").args(["-n", "hw.memsize"]).output().ok()?;
        String::from_utf8(out.stdout).ok()?.trim().parse().ok()
    })
}

#[cfg(not(target_os = "macos"))]
fn installed_memory() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIB: u64 = 1 << 30;

    #[test]
    fn follows_half_of_the_memory_and_never_drops_below_the_default() {
        if cfg!(target_pointer_width = "64") && !cfg!(target_arch = "wasm32") {
            assert_eq!(limit_for(Some(8 * GIB)), 4 * GIB);
            assert_eq!(limit_for(Some(16 * GIB)), 8 * GIB);
            assert_eq!(limit_for(Some(24 * GIB)), 12 * GIB);
            assert_eq!(limit_for(Some(36 * GIB)), 18 * GIB);
            assert_eq!(limit_for(Some(48 * GIB)), 24 * GIB);
            assert_eq!(limit_for(Some(2 * GIB)), DEFAULT_PSD_IMPORT_PIXEL_BYTES);
            assert_eq!(limit_for(None), DEFAULT_PSD_IMPORT_PIXEL_BYTES);
        }
    }
}
