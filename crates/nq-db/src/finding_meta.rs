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

        // ── SMART witness (gated by per-device coverage.can_testify) ──
        "smart_status_lies" => FindingMeta {
            plain_label: "SMART status contradicts error counters",
            operator_label: "SMART Status Lies",
            gloss: "Drive self-reports SMART OVERALL=passed while raw uncorrected \
                    or media error counters are nonzero. SMART self-assessment is \
                    vendor-tuned and stays 'passed' until a manufacturer threshold; \
                    raw counters are the earlier signal. The two channels disagree.",
            contradiction: "The drive says it's healthy. The drive's own raw \
                            counters say it's already producing uncorrected errors. \
                            Both readings came from the same device this cycle.",
            next_checks: &[
                "device path + serial (for replacement ordering)",
                "ZFS / mdraid / filesystem state for the same device — error counters often surface there first",
                "vendor SMART threshold documentation (when does this drive's self-report flip?)",
                "smartctl -a out-of-band for the full attribute table",
            ],
        },

        "smart_uncorrected_errors_nonzero" => FindingMeta {
            plain_label: "Drive reports uncorrected errors",
            operator_label: "SMART Uncorrected Errors",
            gloss: "Raw SCSI uncorrected_read/write/verify counters or NVMe \
                    media_errors are nonzero this cycle. Each entry is a read \
                    or write the device could not deliver or verify reliably. \
                    Level-triggered: fires whenever the count is > 0, not just \
                    when it rises.",
            contradiction: "The drive may still report SMART OVERALL=passed, \
                            and the filesystem may still be serving I/O. \
                            Uncorrected errors are not always immediately \
                            user-visible; they often surface first as \
                            checksum mismatches in ZFS or md scrub events.",
            next_checks: &[
                "ZFS / mdraid / filesystem state for the same device — corruption may already be leaking up",
                "trajectory: are counters rising cycle over cycle, or static?",
                "smart_status_lies: is this drive ALSO co-firing on the contradiction detector?",
                "device path + serial (for replacement ordering)",
            ],
        },

        "smart_witness_silent" => FindingMeta {
            plain_label: "SMART witness stopped reporting",
            operator_label: "SMART Witness Silent",
            gloss: "The nq-witness that provides SMART evidence has gone quiet \
                    or reports status=failed. All SMART-domain detectors gate on \
                    its declared coverage, so silence means drive health is \
                    currently unobserved — not confirmed healthy.",
            contradiction: "A silent witness is not the same as a healthy fleet. \
                            The absence of smart_status_lies / \
                            smart_uncorrected_errors_nonzero findings right now \
                            tells us nothing — the evidence is missing, not clean.",
            next_checks: &[
                "witness helper process status on the publisher",
                "sudoers / privilege grant still intact (sudo_helper mode is the common case)",
                "smartctl binary present on PATH for the witness user",
                "last successful witness collection and its error_message",
                "device tree changes (drives added/removed without witness restart)",
            ],
        },

        "smart_nvme_percentage_used" => FindingMeta {
            plain_label: "NVMe wear approaching projected end-of-life",
            operator_label: "NVMe Wear",
            gloss: "NVMe drive's self-reported percentage_used is at or above \
                    the preventive-replacement threshold (default 80%). The \
                    vendor estimates the drive has consumed this fraction of \
                    its projected program/erase endurance.",
            contradiction: "The drive is still serving I/O. percentage_used is \
                            an endurance estimate, not an immediate failure \
                            signal. The drive may continue working past 100%, \
                            but the vendor stops promising endurance.",
            next_checks: &[
                "drive's nvme_available_spare_pct (separate axis; low spare = remap exhaustion)",
                "nvme_critical_warning bit-field (any flag set is more urgent)",
                "warranty status / replacement parts availability",
                "workload trajectory: writes/day rate vs remaining endurance budget",
            ],
        },

        "smart_nvme_available_spare_low" => FindingMeta {
            plain_label: "NVMe spare blocks running low",
            operator_label: "NVMe Spare Low",
            gloss: "NVMe drive's available_spare_pct has dropped to or below \
                    the floor (default 10%, matching vendor convention). When \
                    spare reaches zero the drive can no longer remap bad \
                    blocks and uncorrected errors begin to surface.",
            contradiction: "Different axis from percentage_used. A drive can \
                            have low wear and still exhaust its spare via \
                            early-life defects, or have high wear with full \
                            spare. Both can fire together; both can fire \
                            independently.",
            next_checks: &[
                "drive's nvme_critical_warning bit 0 (the device sets this when spare drops below its internal threshold; some drives set bit 0 before our 10% floor trips, some after)",
                "trajectory: how fast is spare dropping cycle over cycle?",
                "warranty / replacement parts availability",
                "co-occurring smart_nvme_percentage_used (both axes near limit = harder cliff)",
            ],
        },

        "smart_nvme_critical_warning_set" => FindingMeta {
            plain_label: "NVMe drive raised a critical warning",
            operator_label: "NVMe Critical Warning",
            gloss: "NVMe drive's critical_warning byte has at least one bit set. \
                    This is the device's own active alarm: the drive's internal \
                    logic decided something is wrong. Unlike our percentage_used \
                    or available_spare_pct thresholds, this comes from the drive \
                    itself.",
            contradiction: "The drive may still serve I/O, may still report \
                            smart_overall_passed=true (vendor self-assessment is \
                            tuned conservatively), and may not have crossed our \
                            own thresholds yet. The device disagrees with all of \
                            those and is flagging itself.",
            next_checks: &[
                "decoded bits in the message: spare/temp/reliability/read_only/volatile_backup/persistent_memory each name a specific failure mode",
                "media_read_only or nvm_subsystem_reliability_degraded are the most urgent — schedule replacement now, not later",
                "co-firing siblings: smart_nvme_available_spare_low (bit 0 overlap), smart_nvme_percentage_used (different axis)",
                "warranty / replacement parts availability",
            ],
        },

        "smart_reallocated_sectors_rising" => FindingMeta {
            plain_label: "ATA drive remapping new bad blocks",
            operator_label: "ATA Reallocated Sectors Rising",
            gloss: "ATA drive's reallocated_sector_count strictly increased \
                    since the previous cycle. Edge-triggered: a single \
                    nonzero count is normal factory-baseline; rising in the \
                    field is active media defect emergence.",
            contradiction: "The drive may still report SMART OVERALL=passed; \
                            the filesystem may show no errors yet. Reallocated \
                            sectors are sectors the drive ALREADY successfully \
                            hid — the user-visible read still completed. The \
                            cost is spare pool depletion.",
            next_checks: &[
                "rate of rise: a single increment is normal aging; a burst is the canonical 'replace soon' signal",
                "current_pending_sector and offline_uncorrectable (sibling SMART attributes — pending sectors are the next defects, not yet remapped)",
                "filesystem / md / lvm error counts for the same device — corruption may already be leaking past the device layer",
                "drive's SMART self-assessment vs raw counters (smart_status_lies sibling pattern)",
            ],
        },

        "smart_temperature_high" => FindingMeta {
            plain_label: "Drive running hot",
            operator_label: "SMART Temperature High",
            gloss: "Drive's reported temperature_c is at or above its \
                    class-appropriate warn threshold (NVMe 70°C, SCSI 55°C, \
                    ATA 50°C). Different classes have different operating \
                    ranges — a single threshold would mis-classify either \
                    direction.",
            contradiction: "The drive is still operating, may still report \
                            SMART OVERALL=passed, may not have set its own \
                            critical_warning bit yet. Sustained high temp \
                            often shows up first as I/O latency rising \
                            (drive is throttling), not as failures.",
            next_checks: &[
                "chassis airflow: failed fan, dust buildup, ambient inlet temp",
                "co-firing smart_nvme_critical_warning_set bit 1 (drive's own thermal flag) — if set, drive's internal logic also agrees this is hot",
                "I/O latency / queue depth metrics on the same device — throttling shows up before damage",
                "firmware updates (NVMe vendors occasionally improve thermal management)",
            ],
        },

        // ── Δs: coverage honesty (COVERAGE_HONESTY_GAP V1) ───────
        "coverage_degraded" => FindingMeta {
            plain_label: "Witness coverage materially degraded",
            operator_label: "Coverage Degraded",
            gloss: "The witness is operating and producing fresh evidence, but \
                    the basis behind it is partial in ways its own health check \
                    does not honor. Distinct from staleness (evidence too old) \
                    and from cannot_testify (no standing to look at all): the \
                    evidence is current, the basis is incomplete. Carries a \
                    declared recovery contract — clearance requires sustained \
                    criteria, not a single clean cycle.",
            contradiction: "The producer reports status=ok and is delivering \
                            artifacts on schedule, but a measurable fraction of \
                            the world it claims to observe is being shed at an \
                            internal seam. Operationally up; epistemically \
                            degraded. Acting on the artifacts as full-coverage \
                            evidence is unsound while this is open.",
            next_checks: &[
                "degradation_kind / degradation_metric / degradation_value vs threshold — what shape of partiality, by how much",
                "downstream artifacts produced during this window — they inherit the degradation",
                "recovery_state — active (still bad), candidate (criteria passing, timer running), satisfied (sustained, clearance admissible)",
                "producer's own self-reported health — if green, health_claim_misleading should also be firing",
            ],
        },
        "health_claim_misleading" => FindingMeta {
            plain_label: "Producer health claim contradicts coverage reality",
            operator_label: "Health Claim Misleading",
            gloss: "Composes with a coverage_degraded finding: fires when the \
                    producer self-reports green health while coverage_degraded \
                    is active. Names the P27-shaped gap between the producer's \
                    local correctness and the operator's epistemic standing. \
                    Cannot stand alone — coverage_degraded_ref is required.",
            contradiction: "The producer's /health endpoint is returning ok. \
                            Its own coverage is materially incomplete. Both \
                            statements are simultaneously true on the same \
                            substrate, and the green health claim is not honest \
                            relative to the loss the producer is sustaining.",
            next_checks: &[
                "coverage_degraded_ref — the parent finding carrying the degradation envelope",
                "self_reported_health verbatim from the producer (in message)",
                "whether the producer has a path to surface coverage degradation in its own health output (often: no)",
                "downstream readers consuming the producer's artifacts — they may be silently working from degraded evidence",
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
