//! How much memory this computer has, and whether a model fits. tract builds BiRefNet-lite's
//! optimised plan in about 9 GB (measured on a 24 MP photo: 8.7 to 9.5 GB resident at the peak,
//! whatever the photo's size), and running out of memory there aborts the whole process, so a
//! computer with too little memory is told no before the load, as an [`MlError::LowMemory`].

use crate::{MlError, Model};

/// Total memory below which BiRefNet-lite is not loaded (its peak is about 9 GB).
const BIREFNET_MIN_TOTAL: u64 = 12 << 30;

/// The total memory `model` needs this computer to have, in bytes (0: not measured, no limit).
pub fn min_total_memory(model: Model) -> u64 {
    match model {
        Model::BirefnetLite => BIREFNET_MIN_TOTAL,
        Model::Lama => 0,
    }
}

/// The installed memory in bytes, found once; `None` where it cannot be told (the model then loads).
pub fn physical_memory() -> Option<u64> {
    static TOTAL: std::sync::OnceLock<Option<u64>> = std::sync::OnceLock::new();
    *TOTAL.get_or_init(detect)
}

/// `Ok` when this computer has the memory `model` needs ([`MlError::LowMemory`] otherwise).
pub fn check_memory(model: Model) -> Result<(), MlError> {
    check_total(model, physical_memory())
}

/// [`check_memory`] for a given total (`None`: unknown, so allowed).
pub(crate) fn check_total(model: Model, total: Option<u64>) -> Result<(), MlError> {
    let needs = min_total_memory(model);
    match total {
        Some(have) if have < needs => Err(MlError::LowMemory { have, needs }),
        _ => Ok(()),
    }
}

#[cfg(target_os = "macos")]
fn detect() -> Option<u64> {
    let out = std::process::Command::new("/usr/sbin/sysctl").args(["-n", "hw.memsize"]).output().ok()?;
    String::from_utf8(out.stdout).ok()?.trim().parse().ok()
}

#[cfg(target_os = "linux")]
fn detect() -> Option<u64> {
    let info = std::fs::read_to_string("/proc/meminfo").ok()?;
    let kb: u64 = info.lines().find_map(|l| l.strip_prefix("MemTotal:"))?.trim().trim_end_matches("kB").trim().parse().ok()?;
    kb.checked_mul(1024)
}

#[cfg(target_os = "freebsd")]
fn detect() -> Option<u64> {
    let out = std::process::Command::new("/sbin/sysctl").args(["-n", "hw.physmem"]).output().ok()?;
    String::from_utf8(out.stdout).ok()?.trim().parse().ok()
}

#[cfg(windows)]
fn detect() -> Option<u64> {
    winsafe::GlobalMemoryStatusEx().map(|m| m.ullTotalPhys).ok().filter(|b| *b > 0)
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "freebsd", windows)))]
fn detect() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn birefnet_needs_12_gb_of_total_memory() {
        let gb = |n: u64| Some(n << 30);
        assert_eq!(check_total(Model::BirefnetLite, gb(8)), Err(MlError::LowMemory { have: 8 << 30, needs: 12 << 30 }));
        assert!(check_total(Model::BirefnetLite, gb(11)).is_err());
        assert!(check_total(Model::BirefnetLite, gb(12)).is_ok());
        assert!(check_total(Model::BirefnetLite, gb(64)).is_ok());
        // Not told: the model loads rather than being refused on a guess; LaMa has no limit.
        assert!(check_total(Model::BirefnetLite, None).is_ok());
        assert!(check_total(Model::Lama, gb(1)).is_ok());
    }

    #[test]
    fn the_message_names_both_figures() {
        let e = check_total(Model::BirefnetLite, Some(8 << 30)).unwrap_err().to_string();
        assert!(e.contains("8 GB") && e.contains("12 GB"), "{e}");
    }
}
