use nq_core::wire::{CollectorPayload, HostData};
use nq_core::CollectorStatus;
use time::OffsetDateTime;

pub fn collect() -> CollectorPayload<HostData> {
    let now = OffsetDateTime::now_utc();

    match collect_host_data() {
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
    })
}

fn parse_meminfo_kb(meminfo: &str, key: &str) -> Option<u64> {
    for line in meminfo.lines() {
        if line.starts_with(key) {
            return line
                .split_whitespace()
                .nth(1)
                .and_then(|v| v.parse().ok());
        }
    }
    None
}

fn nix_statvfs(path: &str) -> anyhow::Result<(u64, u64, u64)> {
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
