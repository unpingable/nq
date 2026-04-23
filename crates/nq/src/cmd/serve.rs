use crate::cli::ServeCmd;
use crate::http;
use crate::pull;
use nq_core::config::NotificationChannel;
use nq_core::Config;
use nq_db::{migrate, open_ro, open_rw, publish_batch, update_warning_state, CURRENT_SCHEMA_VERSION};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

pub async fn run(cmd: ServeCmd) -> anyhow::Result<()> {
    let config_text = std::fs::read_to_string(&cmd.config)?;
    let config: Config = serde_json::from_str(&config_text)?;

    let db_path = std::path::PathBuf::from(&config.db_path);

    // Open writer and migrate
    let mut write_db = open_rw(&db_path)?;
    migrate(&mut write_db)?;

    let write_db = Arc::new(Mutex::new(write_db));
    let detector_config = nq_db::DetectorConfig::from(&config.detectors);
    let escalation_config = nq_db::EscalationConfig::from(&config.escalation);
    let bind_addr = config.bind_addr.clone();
    let config = Arc::new(config);

    // Build notification HTTP client once
    let notify_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("notification http client");

    // Start pull loop
    let pull_db = write_db.clone();
    let pull_config = config.clone();
    tokio::spawn(async move {
        let interval = std::time::Duration::from_secs(pull_config.interval_s);
        let mut cycle: u64 = 0;
        loop {
            cycle += 1;
            match pull::pull_all(&pull_config).await {
                Ok(batch) => {
                    let mut db = pull_db.lock().await;
                    match publish_batch(&mut db, &batch) {
                        Ok(result) => {
                            // Run detectors against current state, then update lifecycle
                            match nq_db::detect::run_all(db.conn(), &detector_config) {
                                Ok(findings) => {
                                    if let Err(e) = update_warning_state(
                                        &mut db,
                                        result.generation_id,
                                        &findings,
                                        &escalation_config,
                                    ) {
                                        error!(err = %e, "warning state update failed");
                                    }
                                }
                                Err(e) => {
                                    error!(err = %e, "detector run failed");
                                }
                            }

                            // Compute regime features before sending notifications
                            // so the notifier can annotate payloads with the
                            // current generation's regime badge. If the order
                            // were reversed, each alert would carry features
                            // from the prior pass (or None for brand-new
                            // findings), and the badge would silently lag one
                            // generation behind the dashboard.
                            if let Err(e) = nq_db::compute_features(&mut db, result.generation_id) {
                                warn!(err = %e, "regime feature computation failed");
                            }

                            // Send notifications for escalated findings
                            if !pull_config.notifications.channels.is_empty() {
                                send_notifications(
                                    &mut db,
                                    result.generation_id,
                                    &pull_config.notifications,
                                    &notify_client,
                                ).await;
                            }

                            // Seal generation with content-addressed digest
                            match nq_db::digest::seal_generation(&mut db, result.generation_id) {
                                Ok(hash) => {
                                    info!(
                                        generation = result.generation_id,
                                        ok = result.sources_ok,
                                        failed = result.sources_failed,
                                        hash = %hash,
                                        "published generation"
                                    );
                                }
                                Err(e) => {
                                    error!(err = %e, "generation seal failed");
                                    info!(
                                        generation = result.generation_id,
                                        ok = result.sources_ok,
                                        failed = result.sources_failed,
                                        "published generation (unsealed)"
                                    );
                                }
                            }

                            // Write liveness artifact for the sentinel watcher.
                            // Failure to write is logged but does not crash the publisher —
                            // the primary job is producing generations, not liveness.
                            if let Some(ref liveness_path) = pull_config.liveness.path {
                                let path = std::path::PathBuf::from(liveness_path);
                                let (findings_observed, detectors_run, findings_suppressed): (i64, i64, i64) = db.conn()
                                    .query_row(
                                        "SELECT findings_observed, detectors_run, findings_suppressed
                                         FROM generations WHERE generation_id = ?1",
                                        rusqlite::params![result.generation_id],
                                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                                    )
                                    .unwrap_or((0, 0, 0));
                                let now = time::OffsetDateTime::now_utc()
                                    .format(&time::format_description::well_known::Rfc3339)
                                    .unwrap_or_default();
                                let artifact = nq_db::LivenessArtifact {
                                    liveness_format_version: nq_db::LIVENESS_FORMAT_VERSION,
                                    instance_id: pull_config.liveness.instance_id.clone(),
                                    generated_at: now,
                                    generation_id: result.generation_id,
                                    schema_version: CURRENT_SCHEMA_VERSION,
                                    findings_observed,
                                    findings_suppressed,
                                    detectors_run,
                                    status: "ok".into(),
                                };
                                if let Err(e) = nq_db::write_liveness(&path, &artifact) {
                                    warn!(err = %e, path = %liveness_path, "liveness artifact write failed");
                                }
                            }
                        }
                        Err(e) => {
                            error!(err = %e, "publish failed");
                        }
                    }

                    // Retention pruning
                    if cycle % pull_config.retention.prune_every_n_cycles == 0 {
                        match nq_db::prune(&mut db, pull_config.retention.max_generations) {
                            Ok(stats) if stats.generations_pruned > 0 => {
                                info!(pruned = stats.generations_pruned, "retention prune");
                            }
                            Err(e) => {
                                error!(err = %e, "retention prune failed");
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    error!(err = %e, "pull cycle failed");
                }
            }
            tokio::time::sleep(interval).await;
        }
    });

    // Start HTTP server (with write access for saved queries)
    let read_db = open_ro(&db_path)?;
    info!(bind = %bind_addr, "web UI starting");
    http::serve_with_write(read_db, write_db, &bind_addr).await?;
    Ok(())
}

async fn send_notifications(
    db: &mut nq_db::WriteDb,
    generation_id: i64,
    config: &nq_core::config::NotificationConfig,
    client: &reqwest::Client,
) {
    let pending = match nq_db::notify::find_pending(db, &config.min_severity) {
        Ok(p) => p,
        Err(e) => {
            error!(err = %e, "failed to find pending notifications");
            return;
        }
    };

    if pending.is_empty() {
        return;
    }

    // Group by (host, state_kind, detector_family) before rendering. Lane
    // order (incident → legacy) privileges kind first; severity sorts within
    // a lane. See docs/gaps/ALERT_INTERPRETATION_GAP.md §"State kind as a
    // first-class axis".
    let rollups = nq_db::notify::rollup_pending(pending);
    if rollups.is_empty() {
        return;
    }

    info!(
        count = rollups.len(),
        finding_count = rollups.iter().map(|r| r.findings.len()).sum::<usize>(),
        "sending notifications"
    );

    for r in &rollups {
        let base_url = config.external_url.as_deref().unwrap_or("http://localhost:9848");
        for channel in &config.channels {
            let result = match channel {
                NotificationChannel::Webhook { url, headers } => {
                    let payload = nq_db::notify::build_rollup_webhook_payload(r, generation_id, base_url);
                    let mut req = client.post(url).json(&payload);
                    for (k, v) in headers {
                        req = req.header(k, v);
                    }
                    req.send().await
                }
                NotificationChannel::Slack { webhook_url } => {
                    let payload = nq_db::notify::build_rollup_slack_payload(r, generation_id, base_url);
                    client.post(webhook_url).json(&payload).send().await
                }
                NotificationChannel::Discord { webhook_url } => {
                    let payload = nq_db::notify::build_rollup_discord_payload(r, generation_id, base_url);
                    client.post(webhook_url).json(&payload).send().await
                }
            };

            match result {
                Ok(resp) if resp.status().is_success() => {
                    info!(
                        host = %r.host,
                        state_kind = %r.state_kind.as_str(),
                        detector_family = %r.detector_family,
                        findings = r.findings.len(),
                        "rollup sent"
                    );
                }
                Ok(resp) => {
                    warn!(
                        host = %r.host,
                        state_kind = %r.state_kind.as_str(),
                        status = %resp.status(),
                        "rollup failed"
                    );
                }
                Err(e) => {
                    warn!(
                        host = %r.host,
                        state_kind = %r.state_kind.as_str(),
                        err = %e,
                        "rollup send error"
                    );
                }
            }
        }

        // Mark every finding in the rollup as notified regardless of send
        // success (avoid spam on transient failures). mark_notified is
        // per-(host, kind, subject, severity), not per-rollup.
        for f in &r.findings {
            if let Err(e) = nq_db::notify::mark_notified(db, &f.host, &f.kind, &f.subject, &f.severity) {
                error!(err = %e, "failed to mark notification sent");
            }
        }
    }
}
