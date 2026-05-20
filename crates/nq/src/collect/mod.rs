pub mod host;
pub mod logs;
pub mod prometheus;
pub mod services;
pub mod smart;
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
            smart_witness: Some(smart::collect(config)),
        },
    }
}

fn gethostname() -> String {
    hostname::get()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

#[cfg(test)]
pub(crate) mod test_support {
    //! Shared synchronization for subprocess-spawning tests across
    //! the `collect::*` modules.
    //!
    //! Background: tests in `collect::smart` and `collect::zfs` fork a
    //! helper subprocess (a tiny shell script written into a tempdir)
    //! and parse its stdout. Under the default `cargo test` runner,
    //! these tests run on parallel threads. Subprocess fork on Linux
    //! takes a snapshot of the parent process's memory — *including*
    //! any mutex held by another thread at the moment of fork. The
    //! classic instance is malloc's arena lock: if the child happens
    //! to call malloc before exec'ing the helper, it can deadlock
    //! against a lock that no thread will ever release in the child.
    //! The race surfaces here as the
    //! `collect::{smart,zfs}::tests::schema_mismatch_is_rejected`
    //! flake under heavy parallelism, even though every test passes
    //! in isolation.
    //!
    //! Fix: a single shared mutex that every subprocess-spawning test
    //! takes at entry. This makes the fork+exec windows serial across
    //! `collect::*`'s test surface without changing production code,
    //! without affecting non-spawning tests, and without forcing the
    //! whole test binary to run single-threaded.
    //!
    //! Trade-off: ~10 tests now run serial instead of parallel. Total
    //! suite time grows by the sum of their fork+exec durations
    //! (dominated by `slow_helper_times_out`'s ~2s); acceptable in
    //! exchange for not paying the "trust termite" cost of an
    //! intermittent CI failure that erodes confidence in unrelated
    //! green runs.
    //!
    //! Scope: this is a quarantine of a known parallel-execution
    //! interaction, not a refactor of the helper-collection model.
    //! If we later replace subprocess fork with a different
    //! collection strategy (in-process helper, async I/O, etc.),
    //! this module goes away with it.

    use std::sync::{Mutex, MutexGuard};

    static SUBPROCESS_TEST_LOCK: Mutex<()> = Mutex::new(());

    /// Acquire the shared subprocess-spawn lock. Tolerates poisoning:
    /// a previously-panicked test must not poison the rest of the
    /// suite (the lock guards a fork window, not invariant state).
    pub(crate) fn subprocess_lock() -> MutexGuard<'static, ()> {
        SUBPROCESS_TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner())
    }
}
