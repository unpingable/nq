//! Partial-native BSD host collector (Tier 3a).
//!
//! A shared BSD-ish host-fact reader for macOS and FreeBSD. The two
//! layers are kept apart on purpose:
//!
//! - [`read_bsd_facts`] is the ONLY place that does syscalls (raw
//!   `libc`: `getloadavg`, `statvfs`, `sysctl`). It is `cfg`-gated to
//!   the BSD targets and is therefore lab-verified on the real
//!   substrates, never on Linux CI.
//! - [`assemble_bsd`] is pure: it maps a [`BsdHostFacts`] struct to a
//!   [`HostData`], deciding field-level honesty. It is platform-neutral
//!   and unit-tested on Linux CI with fixtures.
//!
//! Honesty rule: fields a BSD has no honest equivalent for are listed in
//! [`HostData::cannot_testify`] and left `None` — never synthesized. The
//! Linux procfs path is untouched and lives in [`super::host`].
//!
//! Scope: host only. No services, logs, launchd, rc.d, or unified
//! logging. macOS/FreeBSD are NOT claimed as full Tier-3 support.

use nq_core::wire::{CollectorPayload, HostData, HostField};
use nq_core::CollectorStatus;
use time::OffsetDateTime;

/// Which BSD produced a [`BsdHostFacts`] — selects the per-OS deltas
/// (MIB names, boot-id availability). Set by [`read_bsd_facts`] at
/// compile time, or by a test fixture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BsdOs {
    MacOs,
    FreeBsd,
}

/// Platform-neutral intermediate: raw host readings, each `Option`
/// because any individual probe may be unavailable. The shared fields
/// (`loadavg`, `statvfs_root`, `boottime_secs`, `osrelease`) come from
/// identical mechanisms on macOS and FreeBSD; the deltas
/// (`mem_total_bytes` MIB, `boot_uuid` availability) are resolved by the
/// reader per OS.
#[derive(Debug, Clone)]
pub struct BsdHostFacts {
    pub os: BsdOs,
    /// `getloadavg(3)`: (1-minute, 5-minute).
    pub loadavg: Option<(f64, f64)>,
    /// `statvfs("/")`: (block_size, total_blocks, avail_blocks).
    pub statvfs_root: Option<(u64, u64, u64)>,
    /// `sysctl kern.boottime` → boot epoch seconds.
    pub boottime_secs: Option<i64>,
    /// `sysctl kern.osrelease`.
    pub osrelease: Option<String>,
    /// macOS `hw.memsize` / FreeBSD `hw.physmem`.
    pub mem_total_bytes: Option<u64>,
    /// macOS `kern.bootsessionuuid`; `None` on FreeBSD (no per-boot UUID).
    pub boot_uuid: Option<String>,
}

/// Pure assembly of [`BsdHostFacts`] into [`HostData`]. Platform-neutral
/// and side-effect-free (no syscalls, no filesystem) so it is fully
/// testable on Linux CI. `now` is injected for the uptime computation.
///
/// Field-level honesty: `mem_available_mb` and `mem_pressure_pct` have no
/// honest BSD analog and are ALWAYS refused (never synthesized); `boot_id`
/// is refused when the substrate has no per-boot UUID (FreeBSD).
pub fn assemble_bsd(facts: BsdHostFacts, now: OffsetDateTime) -> HostData {
    let mut cannot_testify = Vec::new();

    let (cpu_load_1m, cpu_load_5m) = match facts.loadavg {
        Some((one, five)) => (Some(one), Some(five)),
        None => (None, None),
    };

    let mem_total_mb = facts.mem_total_bytes.map(|b| b / (1024 * 1024));
    // A BSD has no honest 1:1 for MemAvailable, and pressure is derived
    // from it — so both are refused at the field level, never computed
    // the Linux way. This refusal is unconditional on BSD.
    let mem_available_mb = None;
    let mem_pressure_pct = None;
    cannot_testify.push(HostField::MemAvailable);
    cannot_testify.push(HostField::MemPressure);

    let (disk_total_mb, disk_avail_mb, disk_used_pct) = match facts.statvfs_root {
        Some((frsize, blocks, bavail)) => {
            let total = blocks.saturating_mul(frsize) / (1024 * 1024);
            let avail = bavail.saturating_mul(frsize) / (1024 * 1024);
            let used_pct = if blocks > 0 {
                Some(((blocks - bavail) as f64 / blocks as f64) * 100.0)
            } else {
                None
            };
            (Some(total), Some(avail), used_pct)
        }
        None => (None, None, None),
    };

    let uptime_seconds = facts.boottime_secs.and_then(|bt| {
        let up = now.unix_timestamp() - bt;
        if up >= 0 {
            Some(up as u64)
        } else {
            None
        }
    });

    let kernel_version = facts.osrelease.clone();

    let boot_id = facts.boot_uuid.clone();
    if boot_id.is_none() {
        // FreeBSD exposes no per-boot UUID (kern.hostuuid is per-host).
        // Refuse rather than fabricate boot identity.
        cannot_testify.push(HostField::BootId);
    }

    HostData {
        cpu_load_1m,
        cpu_load_5m,
        mem_total_mb,
        mem_available_mb,
        mem_pressure_pct,
        disk_total_mb,
        disk_avail_mb,
        disk_used_pct,
        uptime_seconds,
        kernel_version,
        boot_id,
        cannot_testify,
    }
}

// ---------------------------------------------------------------------------
// Collector entry point. On a BSD target it reads facts and assembles a
// supported-partial payload (status: ok + cannot_testify). On any other
// target the BSD fact reader is not compiled, so it reports typed
// incapacity — reached only if Platform::MacOs/FreeBsd dispatch happens on
// a non-BSD build (e.g. an injected test on Linux CI).
// ---------------------------------------------------------------------------

#[cfg(any(target_os = "macos", target_os = "freebsd"))]
pub fn bsd_collect(now: OffsetDateTime) -> CollectorPayload<HostData> {
    CollectorPayload {
        status: CollectorStatus::Ok,
        collected_at: Some(now),
        error_message: None,
        data: Some(assemble_bsd(read_bsd_facts(), now)),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "freebsd")))]
pub fn bsd_collect(now: OffsetDateTime) -> CollectorPayload<HostData> {
    CollectorPayload {
        status: CollectorStatus::NotSupported,
        collected_at: Some(now),
        error_message: None,
        data: None,
    }
}

// ---------------------------------------------------------------------------
// Syscall layer — BSD targets only. Lab-verified on the FreeBSD VM and the
// mac mini; never exercised on Linux CI (not compiled here).
// ---------------------------------------------------------------------------

#[cfg(any(target_os = "macos", target_os = "freebsd"))]
pub fn read_bsd_facts() -> BsdHostFacts {
    BsdHostFacts {
        os: current_bsd_os(),
        loadavg: read_loadavg(),
        // statvfs is POSIX and identical across the BSDs and Linux; reuse
        // the shared implementation rather than duplicating it.
        statvfs_root: super::host::nix_statvfs("/").ok(),
        boottime_secs: read_boottime_secs(),
        osrelease: sysctl_string("kern.osrelease"),
        mem_total_bytes: read_mem_total_bytes(),
        boot_uuid: read_boot_uuid(),
    }
}

#[cfg(target_os = "macos")]
fn current_bsd_os() -> BsdOs {
    BsdOs::MacOs
}
#[cfg(target_os = "freebsd")]
fn current_bsd_os() -> BsdOs {
    BsdOs::FreeBsd
}

#[cfg(any(target_os = "macos", target_os = "freebsd"))]
fn read_loadavg() -> Option<(f64, f64)> {
    let mut avg = [0.0f64; 3];
    let n = unsafe { libc::getloadavg(avg.as_mut_ptr(), 3) };
    if n >= 2 {
        Some((avg[0], avg[1]))
    } else {
        None
    }
}

// macOS: hw.memsize (int64). FreeBSD: hw.physmem (long). Both 8 bytes on
// the amd64/arm64 lab targets.
#[cfg(target_os = "macos")]
fn read_mem_total_bytes() -> Option<u64> {
    sysctl_u64("hw.memsize")
}
#[cfg(target_os = "freebsd")]
fn read_mem_total_bytes() -> Option<u64> {
    sysctl_u64("hw.physmem")
}

#[cfg(target_os = "macos")]
fn read_boot_uuid() -> Option<String> {
    sysctl_string("kern.bootsessionuuid")
}
#[cfg(target_os = "freebsd")]
fn read_boot_uuid() -> Option<String> {
    // FreeBSD has no per-boot UUID; assemble_bsd refuses BootId.
    None
}

#[cfg(any(target_os = "macos", target_os = "freebsd"))]
fn read_boottime_secs() -> Option<i64> {
    use std::ffi::CString;
    let cname = CString::new("kern.boottime").ok()?;
    let mut tv = libc::timeval {
        tv_sec: 0,
        tv_usec: 0,
    };
    let mut len = std::mem::size_of::<libc::timeval>() as libc::size_t;
    let r = unsafe {
        libc::sysctlbyname(
            cname.as_ptr(),
            (&mut tv as *mut libc::timeval).cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if r == 0 {
        Some(tv.tv_sec as i64)
    } else {
        None
    }
}

#[cfg(any(target_os = "macos", target_os = "freebsd"))]
fn sysctl_u64(name: &str) -> Option<u64> {
    use std::ffi::CString;
    let cname = CString::new(name).ok()?;
    let mut val: u64 = 0;
    let mut len = std::mem::size_of::<u64>() as libc::size_t;
    let r = unsafe {
        libc::sysctlbyname(
            cname.as_ptr(),
            (&mut val as *mut u64).cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if r == 0 {
        Some(val)
    } else {
        None
    }
}

#[cfg(any(target_os = "macos", target_os = "freebsd"))]
fn sysctl_string(name: &str) -> Option<String> {
    use std::ffi::CString;
    let cname = CString::new(name).ok()?;
    let mut len: libc::size_t = 0;
    // Query the size first (null buffer).
    let r = unsafe {
        libc::sysctlbyname(
            cname.as_ptr(),
            std::ptr::null_mut(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if r != 0 || len == 0 {
        return None;
    }
    let mut buf = vec![0u8; len];
    let r = unsafe {
        libc::sysctlbyname(
            cname.as_ptr(),
            buf.as_mut_ptr().cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if r != 0 {
        return None;
    }
    // `len` includes the trailing NUL; drop it.
    if len > 0 && buf.get(len - 1) == Some(&0) {
        buf.truncate(len - 1);
    } else {
        buf.truncate(len);
    }
    String::from_utf8(buf).ok().map(|s| s.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn darwin_facts() -> BsdHostFacts {
        BsdHostFacts {
            os: BsdOs::MacOs,
            loadavg: Some((1.5, 1.2)),
            statvfs_root: Some((4096, 1_000_000, 400_000)),
            boottime_secs: Some(1_700_000_000),
            osrelease: Some("24.0.0".into()),
            mem_total_bytes: Some(16 * 1024 * 1024 * 1024),
            boot_uuid: Some("11111111-2222-3333-4444-555555555555".into()),
        }
    }

    fn freebsd_facts() -> BsdHostFacts {
        BsdHostFacts {
            os: BsdOs::FreeBsd,
            loadavg: Some((0.3, 0.2)),
            statvfs_root: Some((4096, 2_000_000, 1_000_000)),
            boottime_secs: Some(1_700_000_000),
            osrelease: Some("14.4-RELEASE".into()),
            mem_total_bytes: Some(4u64 * 1024 * 1024 * 1024),
            boot_uuid: None,
        }
    }

    fn fixed_now() -> OffsetDateTime {
        // 3600s after the fixtures' boottime.
        OffsetDateTime::from_unix_timestamp(1_700_003_600).unwrap()
    }

    #[test]
    fn darwin_assembles_shared_fields_and_refuses_only_mem() {
        let h = assemble_bsd(darwin_facts(), fixed_now());
        assert_eq!(h.cpu_load_1m, Some(1.5));
        assert_eq!(h.cpu_load_5m, Some(1.2));
        assert_eq!(h.mem_total_mb, Some(16 * 1024));
        assert_eq!(h.disk_total_mb, Some(1_000_000u64 * 4096 / (1024 * 1024)));
        assert_eq!(h.uptime_seconds, Some(3600));
        assert_eq!(h.kernel_version.as_deref(), Some("24.0.0"));
        // Darwin HAS a per-boot UUID, so boot_id is populated and NOT refused.
        assert_eq!(
            h.boot_id.as_deref(),
            Some("11111111-2222-3333-4444-555555555555")
        );
        assert!(!h.cannot_testify.contains(&HostField::BootId));
        // Only the two memory fields are refused.
        assert_eq!(
            h.cannot_testify,
            vec![HostField::MemAvailable, HostField::MemPressure]
        );
        assert!(h.mem_available_mb.is_none());
        assert!(h.mem_pressure_pct.is_none());
    }

    #[test]
    fn freebsd_refuses_boot_id_and_mem() {
        let h = assemble_bsd(freebsd_facts(), fixed_now());
        // FreeBSD has no per-boot UUID: boot_id is None AND refused.
        assert!(h.boot_id.is_none());
        assert_eq!(
            h.cannot_testify,
            vec![
                HostField::MemAvailable,
                HostField::MemPressure,
                HostField::BootId
            ]
        );
        // Shared fields still assemble honestly.
        assert_eq!(h.kernel_version.as_deref(), Some("14.4-RELEASE"));
        assert_eq!(h.cpu_load_1m, Some(0.3));
        assert_eq!(h.uptime_seconds, Some(3600));
    }

    #[test]
    fn never_synthesizes_mem_available_or_pressure() {
        // Counterfeit guard: even with total memory present, available and
        // pressure stay None and are refused — never derived the Linux way.
        // This test fails loudly if someone later "helpfully" computes them.
        let mut f = darwin_facts();
        f.mem_total_bytes = Some(99 * 1024 * 1024 * 1024);
        let h = assemble_bsd(f, fixed_now());
        assert!(h.mem_available_mb.is_none());
        assert!(h.mem_pressure_pct.is_none());
        assert!(h.cannot_testify.contains(&HostField::MemAvailable));
        assert!(h.cannot_testify.contains(&HostField::MemPressure));
    }

    #[test]
    fn assemble_is_pure_and_never_references_proc_fs() {
        // Structural guarantee: the BSD path must never read the Linux
        // proc filesystem. Build the needle by concat so this assertion
        // is not its own counterexample.
        let src = include_str!("host_bsd.rs");
        let needle = ["/pr", "oc"].concat();
        assert!(
            !src.contains(&needle),
            "the BSD host collector must not reference the Linux proc fs"
        );
    }
}
