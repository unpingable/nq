//! Canonical finding metadata: one source of truth, multiple renderings.
//!
//! Each finding kind has:
//! - a plain-English label (leads in UI, docs, notifications)
//! - an operator label (compact, for people who already speak Δ)
//! - a one-sentence gloss (why this matters)
//! - a contradiction template (what makes this more than a threshold alert)
//! - suggested next checks

/// Static metadata for a finding kind.
pub struct FindingMeta {
    /// Primary label: plain English, no Greek. Leads in the UI.
    pub plain_label: &'static str,
    /// Operator-compact label for the Δ-native.
    pub operator_label: &'static str,
    /// One-sentence gloss: why this matters beyond the obvious metric.
    pub gloss: &'static str,
    /// What makes this finding a contradiction, not just a threshold.
    pub contradiction: &'static str,
    /// Suggested next checks.
    pub next_checks: &'static [&'static str],
}

/// Domain metadata for the four failure domains.
pub struct DomainMeta {
    pub code: &'static str,
    pub operator_label: &'static str,
    pub plain_label: &'static str,
    pub gloss: &'static str,
}

pub const DOMAINS: &[DomainMeta] = &[
    DomainMeta {
        code: "Δo",
        operator_label: "missing",
        plain_label: "Signal stopped arriving",
        gloss: "Something that was reporting has gone quiet. The absence itself is the evidence.",
    },
    DomainMeta {
        code: "Δs",
        operator_label: "skewed",
        plain_label: "Signal present but untrustworthy",
        gloss: "Data is arriving, but it no longer correlates with reality.",
    },
    DomainMeta {
        code: "Δg",
        operator_label: "unstable",
        plain_label: "Substrate under pressure",
        gloss: "The service may look up, but the medium underneath it is struggling.",
    },
    DomainMeta {
        code: "Δh",
        operator_label: "degrading",
        plain_label: "Worsening over time",
        gloss: "Technically within spec right now, but trending toward a failure state.",
    },
];

pub fn domain_meta(code: &str) -> Option<&'static DomainMeta> {
    DOMAINS.iter().find(|d| d.code == code)
}

/// Look up canonical metadata for a finding kind.
pub fn finding_meta(kind: &str) -> FindingMeta {
    match kind {
        // ── Δg: unstable ──────────────────────────────────────────
        "wal_bloat" => FindingMeta {
            plain_label: "Storage layer under stress",
            operator_label: "Substrate Pressure",
            gloss: "The WAL is growing faster than checkpoints can retire it. \
                    This kind of failure appears in persistence behavior before it shows up in availability or logs.",
            contradiction: "Service is up and app health is nominal, but write geometry is worsening underneath.",
            next_checks: &["WAL growth vs DB size", "checkpoint/compaction lag", "time to disk full"],
        },
        "freelist_bloat" => FindingMeta {
            plain_label: "Wasted storage accumulating",
            operator_label: "Substrate Pressure",
            gloss: "Deleted rows left behind free pages that aren't being reclaimed. \
                    The database is larger than it needs to be and growing.",
            contradiction: "Retention is running and deleting rows, but the database isn't shrinking.",
            next_checks: &["auto_vacuum setting", "VACUUM schedule", "freelist page count vs total"],
        },
        "disk_pressure" => FindingMeta {
            plain_label: "Disk nearing capacity",
            operator_label: "Substrate Pressure",
            gloss: "Disk is above 90% used. Writes will start failing before availability metrics notice.",
            contradiction: "Services still report healthy. Disk exhaustion appears first in geometry, not in app errors.",
            next_checks: &["largest files and growth rate", "retention/cleanup jobs", "time to full"],
        },
        "mem_pressure" => FindingMeta {
            plain_label: "Memory under pressure",
            operator_label: "Substrate Pressure",
            gloss: "Memory pressure is above 85%. The OOM killer may intervene before the service notices.",
            contradiction: "Service is up, but the operating system is under allocation pressure.",
            next_checks: &["top memory consumers", "swap usage", "recent memory growth"],
        },
        "service_status" => FindingMeta {
            plain_label: "Service down or degraded",
            operator_label: "Service Failure",
            gloss: "A monitored service is not in the expected running state.",
            contradiction: "The host is up and reachable, but this specific service is not running normally.",
            next_checks: &["service logs", "recent restarts", "dependency status"],
        },

        // ── Δo: missing ──────────────────────────────────────────
        "stale_host" => FindingMeta {
            plain_label: "Host stopped reporting",
            operator_label: "Missing Observable",
            gloss: "No fresh data from this host for multiple collection cycles. \
                    It may be down, unreachable, or the publisher may have stopped.",
            contradiction: "The host may still be running. We can't tell — that's the problem.",
            next_checks: &["network connectivity", "publisher process status", "last source run"],
        },
        "stale_service" => FindingMeta {
            plain_label: "Service data stopped arriving",
            operator_label: "Missing Observable",
            gloss: "Service information is stale. The data we're showing is from a previous collection cycle.",
            contradiction: "The host is reporting, but this service's data has stopped updating.",
            next_checks: &["service collector status", "collector error messages", "service process"],
        },
        "signal_dropout" => FindingMeta {
            plain_label: "Signal vanished",
            operator_label: "Signal Dropout",
            gloss: "A metric series or service that was consistently present has disappeared. \
                    This is different from a value going to zero — the signal itself is gone.",
            contradiction: "The collection pipeline is working. This specific signal just stopped existing.",
            next_checks: &["exporter configuration", "service rename or removal", "scrape target health"],
        },
        "log_silence" => FindingMeta {
            plain_label: "Log source went quiet",
            operator_label: "Observability Gap",
            gloss: "A log source that normally produces output has gone silent. \
                    Silence from a running service is itself a signal.",
            contradiction: "The service is up and the log transport is working, but no lines are arriving.",
            next_checks: &["log rotation", "service activity", "transport lag"],
        },
        "scrape_regime_shift" => FindingMeta {
            plain_label: "Metric collection shifted",
            operator_label: "Regime Shift",
            gloss: "A large fraction of metric series appeared or vanished between generations. \
                    Something changed in what's being measured, not just the measurements.",
            contradiction: "The scrape target is responding, but the shape of what it reports has changed.",
            next_checks: &["exporter version or config change", "service restart", "new or removed components"],
        },

        // ── Δs: skewed ───────────────────────────────────────────
        "source_error" => FindingMeta {
            plain_label: "Collection failing",
            operator_label: "Source Error",
            gloss: "The aggregator cannot reach or parse data from this publisher. \
                    All downstream analysis for this source is based on stale data.",
            contradiction: "The host may be fine. We just can't see it right now.",
            next_checks: &["publisher process", "network path", "last successful collection"],
        },
        "metric_signal" => FindingMeta {
            plain_label: "Corrupted metric values",
            operator_label: "Signal Corruption",
            gloss: "Metric values include NaN or infinity. Downstream aggregation and alerting \
                    will produce nonsense results.",
            contradiction: "The exporter is responding and the metric exists, but the value is meaningless.",
            next_checks: &["exporter health", "upstream data source", "metric computation logic"],
        },
        "error_shift" => FindingMeta {
            plain_label: "Error rate spiked",
            operator_label: "Error Regime Change",
            gloss: "Log error rate jumped significantly above baseline. \
                    The application is producing errors at an unusual rate.",
            contradiction: "The service may still be 'up' by health check standards, but its error output tells a different story.",
            next_checks: &["error log examples", "recent deployments", "upstream dependency health"],
        },
        "check_error" => FindingMeta {
            plain_label: "Check failed to execute",
            operator_label: "Meta: Check Error",
            gloss: "A saved query check encountered an error. The check itself is broken, \
                    so the condition it monitors is unobserved.",
            contradiction: "The monitoring system is running, but this specific check can't do its job.",
            next_checks: &["check SQL syntax", "referenced tables exist", "schema version"],
        },

        // ── Δh: degrading ────────────────────────────────────────
        "resource_drift" => FindingMeta {
            plain_label: "Resource usage trending worse",
            operator_label: "Drift",
            gloss: "CPU, memory, or disk is consistently above its recent average. \
                    Not an emergency yet, but the trend line is moving the wrong direction.",
            contradiction: "Current values are within limits, but the historical average shows sustained increase.",
            next_checks: &["what changed recently", "growth rate vs capacity", "workload changes"],
        },
        "service_flap" => FindingMeta {
            plain_label: "Service oscillating",
            operator_label: "Flap",
            gloss: "A service is cycling between states. It keeps restarting, which makes \
                    'up' and 'down' both misleading descriptions.",
            contradiction: "The service is technically 'up' right now, but it's been bouncing. Uptime is a lie.",
            next_checks: &["restart count and timing", "crash logs", "resource exhaustion at restart"],
        },

        // ── ZFS witness (gated by coverage.can_testify) ──────────
        "zfs_pool_degraded" => FindingMeta {
            plain_label: "ZFS pool redundancy degraded",
            operator_label: "ZFS Degraded",
            gloss: "A ZFS pool is in state DEGRADED. A drive or vdev is faulted; \
                    the pool still serves data but redundancy is narrower than configured. \
                    Regime features distinguish chronic-stable from actively worsening.",
            contradiction: "The pool is mounted and responding. Reads and writes complete. \
                            That's exactly what makes this dangerous — a second failure \
                            before repair can cross the line to unavailable or lossy.",
            next_checks: &[
                "which vdev is faulted, how long, error counters trajectory",
                "spare assignment and activation status",
                "last scrub completion and whether it found errors",
            ],
        },
        "zfs_vdev_faulted" => FindingMeta {
            plain_label: "ZFS vdev failed",
            operator_label: "ZFS Vdev Faulted",
            gloss: "A specific device in a ZFS pool is in state FAULTED or UNAVAIL. \
                    The pool's redundancy still protects data if other vdevs are healthy, \
                    but the failure surface has narrowed. Multiple faulted vdevs in one \
                    pool exhaust redundancy and escalate to ImmediateRisk.",
            contradiction: "The pool reports DEGRADED (not FAULTED) — data is still \
                            accessible — but this specific device is gone. The pool is \
                            functioning on reduced redundancy; a second device failure \
                            may cross into data loss.",
            next_checks: &[
                "device path + GUID (for replacement ordering)",
                "error counter trajectory across generations",
                "spare availability and whether one has activated",
                "SMART data out-of-band (witness does not include it)",
            ],
        },
        "zfs_error_count_increased" => FindingMeta {
            plain_label: "ZFS vdev errors rising",
            operator_label: "ZFS Error Count Rise",
            gloss: "Read, write, or checksum error counters on a specific vdev \
                    strictly rose since the previous cycle. Edge-triggered — \
                    fires on the rise itself, not on persistent elevation.",
            contradiction: "The pool may still report OK or DEGRADED without change; \
                            the vdev may still be ONLINE. The detector catches the \
                            trajectory, not the current-state label. Rising errors \
                            precede state changes.",
            next_checks: &[
                "which counter rose (read / write / checksum)",
                "magnitude of the rise vs prior cycles",
                "device path + model (for SMART cross-reference out-of-band)",
                "scrub schedule — a rise during scrub means the pool is finding bad data",
            ],
        },
        "zfs_witness_silent" => FindingMeta {
            plain_label: "ZFS witness stopped reporting",
            operator_label: "ZFS Witness Silent",
            gloss: "The nq-witness that provides ZFS evidence has gone quiet or reports \
                    status=failed. All ZFS-domain detectors gate on its declared coverage, \
                    so silence means pool health is currently unobserved.",
            contradiction: "A silent witness is not the same as a healthy pool. The \
                            absence of degraded-pool findings right now tells us \
                            nothing — the evidence is missing, not clean.",
            next_checks: &[
                "witness helper process status on the publisher",
                "sudoers / privilege grant still intact (if sudo_helper mode)",
                "last successful witness collection and its error_message",
            ],
        },

        // ── meta ─────────────────────────────────────────────────
        "check_failed" => FindingMeta {
            plain_label: "Check condition detected",
            operator_label: "Meta: Check",
            gloss: "A user-defined SQL check returned results that indicate a problem.",
            contradiction: "Standard detectors didn't catch this. A custom check did.",
            next_checks: &["check query definition", "result rows", "check threshold"],
        },

        // Fallback for unknown kinds
        _ => FindingMeta {
            plain_label: "Unknown finding",
            operator_label: "Unknown",
            gloss: "An unrecognized finding type.",
            contradiction: "",
            next_checks: &[],
        },
    }
}
