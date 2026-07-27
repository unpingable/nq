use super::host_bsd;
use nq_core::wire::{CollectorPayload, HostData};
use nq_core::{CollectorStatus, Platform};
use time::OffsetDateTime;

pub fn collect() -> CollectorPayload<HostData> {
    collect_for(Platform::current())
}

/// Host collection, dispatched on the substrate (injectable so dispatch
/// is testable on Linux CI):
/// - **Linux** — reads `/proc` (unchanged reference path).
/// - **macOS / FreeBSD** — partial-native BSD host collector
///   ([`super::host_bsd`]): `status: ok` plus a `cannot_testify` list
///   for fields with no honest BSD equivalent. The BSD fact-reading is
///   cfg-gated; on a non-BSD build target the dispatch falls through to
///   [`CollectorStatus::NotSupported`].
/// - **Other** (e.g. Windows) — typed [`CollectorStatus::NotSupported`].
pub fn collect_for(platform: Platform) -> CollectorPayload<HostData> {
    let now = OffsetDateTime::now_utc();

    match platform {
        Platform::Linux => match collect_host_data() {
            Ok(data) => CollectorPayload {
                status: CollectorStatus::Ok,
                collected_at: Some(now),
                error_message: None,
                data: Some(data),
            },
            Err(e) => CollectorPayload {
                status: CollectorStatus::Error,
                collected_at: Some(now),
                error_message: Some(e.to_string()),
                data: None,
            },
        },
        Platform::MacOs | Platform::FreeBsd => host_bsd::bsd_collect(now),
        Platform::Other => CollectorPayload {
            status: CollectorStatus::NotSupported,
            collected_at: Some(now),
            error_message: None,
            data: None,
        },
    }
}

fn collect_host_data() -> anyhow::Result<HostData> {
    // Load averages
    let loadavg = std::fs::read_to_string("/proc/loadavg")?;
    let parts: Vec<&str> = loadavg.split_whitespace().collect();
    let cpu_load_1m = parts.first().and_then(|s| s.parse::<f64>().ok());
    let cpu_load_5m = parts.get(1).and_then(|s| s.parse::<f64>().ok());

    // Memory from /proc/meminfo
    let meminfo = std::fs::read_to_string("/proc/meminfo")?;
    let mem_total_mb = parse_meminfo_kb(&meminfo, "MemTotal").map(|kb| kb / 1024);
    let mem_available_mb = parse_meminfo_kb(&meminfo, "MemAvailable").map(|kb| kb / 1024);
    let mem_pressure_pct = match (mem_total_mb, mem_available_mb) {
        (Some(total), Some(avail)) if total > 0 => {
            Some(((total - avail) as f64 / total as f64) * 100.0)
        }
        _ => None,
    };

    // Disk usage for root filesystem
    let statvfs = nix_statvfs("/")?;
    let block_size = statvfs.0;
    let total_blocks = statvfs.1;
    let avail_blocks = statvfs.2;
    let disk_total_mb = Some(total_blocks * block_size / (1024 * 1024));
    let disk_avail_mb = Some(avail_blocks * block_size / (1024 * 1024));
    let disk_used_pct = if total_blocks > 0 {
        Some(((total_blocks - avail_blocks) as f64 / total_blocks as f64) * 100.0)
    } else {
        None
    };

    // Uptime
    let uptime_str = std::fs::read_to_string("/proc/uptime")?;
    let uptime_seconds = uptime_str
        .split_whitespace()
        .next()
        .and_then(|s| s.parse::<f64>().ok())
        .map(|s| s as u64);

    // Kernel
    let kernel_version = std::fs::read_to_string("/proc/version")
        .ok()
        .and_then(|v| v.split_whitespace().nth(2).map(String::from));

    // Boot ID
    let boot_id = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .ok()
        .map(|s| s.trim().to_string());

    Ok(HostData {
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
        // Linux testifies to every host field via /proc + statvfs.
        cannot_testify: Vec::new(),
    })
}

fn parse_meminfo_kb(meminfo: &str, key: &str) -> Option<u64> {
    for line in meminfo.lines() {
        if line.starts_with(key) {
            return line.split_whitespace().nth(1).and_then(|v| v.parse().ok());
        }
    }
    None
}

/// `(block_size, total_blocks, avail_blocks)` for `path` via POSIX
/// `statvfs`. Shared with the BSD host collector — `statvfs` is portable
/// across Linux and the BSDs, so the disk path is identical everywhere.
pub(crate) fn nix_statvfs(path: &str) -> anyhow::Result<(u64, u64, u64)> {
    // (block_size, total_blocks, avail_blocks)
    // Use libc::statvfs directly to avoid heavy deps
    use std::ffi::CString;
    use std::mem::MaybeUninit;

    let c_path = CString::new(path)?;
    let mut buf = MaybeUninit::<libc::statvfs>::uninit();

    let ret = unsafe { libc::statvfs(c_path.as_ptr(), buf.as_mut_ptr()) };
    if ret != 0 {
        return Err(anyhow::anyhow!(
            "statvfs failed: {}",
            std::io::Error::last_os_error()
        ));
    }

    let stat = unsafe { buf.assume_init() };
    Ok((
        stat.f_frsize as u64,
        stat.f_blocks as u64,
        stat.f_bavail as u64,
    ))
}

#[cfg(test)]
mod platform_tests {
    use super::*;

    #[test]
    fn parse_meminfo_kb_extracts_named_key_value() {
        let meminfo = "MemTotal:       16384256 kB\nMemAvailable:    8192000 kB\n";
        assert_eq!(parse_meminfo_kb(meminfo, "MemTotal"), Some(16_384_256));
        assert_eq!(parse_meminfo_kb(meminfo, "MemAvailable"), Some(8_192_000));
    }

    #[test]
    fn parse_meminfo_kb_refuses_missing_or_malformed_values() {
        let meminfo = "MemTotal: not-a-number kB\nSwapTotal: 42 kB\n";
        assert_eq!(parse_meminfo_kb(meminfo, "MemTotal"), None);
        assert_eq!(parse_meminfo_kb(meminfo, "MemAvailable"), None);
    }

    #[test]
    fn other_platform_is_not_supported_not_error() {
        // `Other` (e.g. Windows) is the truly-unsupported arm. macOS and
        // FreeBSD now dispatch to the partial-native BSD collector and are
        // exercised in `host_bsd` via fixtures, not here.
        let p = collect_for(Platform::Other);
        assert_eq!(p.status, CollectorStatus::NotSupported);
        assert_ne!(
            p.status,
            CollectorStatus::Error,
            "incapacity must not launder into a generic error"
        );
        assert!(
            p.data.is_none(),
            "an unsupported substrate must not produce host data (green silence)"
        );
        assert!(
            p.error_message.is_none(),
            "not_supported is incapacity, not an error string"
        );
    }

    #[test]
    fn linux_substrate_path_unchanged() {
        // On the Linux reference substrate the collector reads /proc and
        // testifies Ok; the platform gate must not alter that. Asserted
        // only where CI actually runs Linux.
        let p = collect_for(Platform::Linux);
        if cfg!(target_os = "linux") {
            assert_eq!(p.status, CollectorStatus::Ok, "payload: {p:?}");
            assert!(p.data.is_some());
        }
    }

    #[test]
    fn collect_on_a_supported_substrate_is_ok() {
        // Live smoke against whatever substrate runs the suite. On Linux
        // this drives /proc; on the macOS/FreeBSD lab substrates this is
        // the ONLY in-suite exercise of the cfg-gated BSD fact reader
        // (read_bsd_facts), which Linux CI cannot compile. All three
        // supported substrates must return Ok with data.
        if cfg!(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "freebsd"
        )) {
            let p = collect();
            assert_eq!(p.status, CollectorStatus::Ok, "payload: {p:?}");
            let data = p.data.expect("supported substrate must produce host data");
            // On the BSDs the partial-native collector must refuse the
            // memory fields it has no honest equivalent for — proving the
            // real fact reader produced a cannot_testify list, not silence.
            if cfg!(any(target_os = "macos", target_os = "freebsd")) {
                use nq_core::wire::HostField;
                assert!(
                    data.cannot_testify.contains(&HostField::MemAvailable)
                        && data.cannot_testify.contains(&HostField::MemPressure),
                    "BSD host must refuse mem_available/mem_pressure: {:?}",
                    data.cannot_testify
                );
            }
        }
    }
}
