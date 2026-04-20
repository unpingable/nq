pub mod host;
pub mod logs;
pub mod prometheus;
pub mod services;
pub mod sqlite_health;
pub mod zfs;

use nq_core::wire::{Collectors, PublisherState};
use nq_core::PublisherConfig;
use time::OffsetDateTime;

/// Collect all local state and return the publisher wire format.
pub fn collect_state(config: &PublisherConfig) -> PublisherState {
    let hostname = gethostname();
    let now = OffsetDateTime::now_utc();

    PublisherState {
        host: hostname,
        collected_at: now,
        collectors: Collectors {
            host: Some(host::collect()),
            services: Some(services::collect(config)),
            sqlite_health: Some(sqlite_health::collect(config)),
            prometheus: Some(prometheus::collect(config)),
            logs: Some(logs::collect(config)),
            zfs_witness: Some(zfs::collect(config)),
        },
    }
}

fn gethostname() -> String {
    hostname::get()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}
