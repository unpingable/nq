//! Sentinel: out-of-band liveness witness for NQ.
//!
//! Reads the liveness artifact NQ publishes after each generation and
//! alerts on staleness/silence. Deliberately dumb: does not import
//! detector logic, does not touch nq.db, depends only on the artifact.
//!
//! See docs/gaps/SENTINEL_LIVENESS_GAP.md.

use crate::cli::SentinelCmd;
use nq_core::config::NotificationChannel;
use nq_db::{read_liveness, LivenessArtifact, LivenessReadError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tracing::{error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentinelConfig {
    /// Path to the liveness artifact file.
    pub artifact_path: String,
    /// Max age (seconds) of the artifact before marking stale.
    #[serde(default = "default_max_age_secs")]
    pub max_age_seconds: i64,
    /// How often to check, in seconds.
    #[serde(default = "default_poll_interval_secs")]
    pub poll_interval_seconds: u64,
    /// Grace period on startup (seconds) before alerting on missing artifact.
    #[serde(default = "default_grace_secs")]
    pub grace_period_seconds: u64,
    /// How many consecutive polls with the same generation_id before marking stuck.
    /// Only meaningful if the timestamp is advancing but generation_id isn't.
    #[serde(default = "default_stuck_polls")]
    pub stuck_after_polls: u64,
    /// Alert channels. Reuses NotificationChannel from the main config.
    #[serde(default)]
    pub channels: Vec<NotificationChannel>,
}

fn default_max_age_secs() -> i64 { 180 }
fn default_poll_interval_secs() -> u64 { 60 }
fn default_grace_secs() -> u64 { 120 }
fn default_stuck_polls() -> u64 { 5 }

/// Observed liveness state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Healthy,
    Stale,
    Stuck,
    Missing,
    Malformed,
    /// Grace period after startup; don't alert yet.
    Starting,
}

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Stale => "stale",
            Self::Stuck => "stuck",
            Self::Missing => "missing",
            Self::Malformed => "malformed",
            Self::Starting => "starting",
        }
    }
}

/// Compute the state given an observed artifact (or read error) and context.
pub fn classify(
    now: OffsetDateTime,
    max_age_seconds: i64,
    artifact: &Result<LivenessArtifact, LivenessReadError>,
    last_generation_id: Option<i64>,
    same_gen_run: u64,
    stuck_after_polls: u64,
) -> State {
    match artifact {
        Err(LivenessReadError::Missing) => State::Missing,
        Err(LivenessReadError::Malformed(_)) => State::Malformed,
        Err(LivenessReadError::Io(_)) => State::Missing, // treat IO errors as missing
        Ok(a) => {
            // Parse the timestamp
            let generated_at = match OffsetDateTime::parse(&a.generated_at, &Rfc3339) {
                Ok(t) => t,
                Err(_) => return State::Malformed,
            };
            let age = (now - generated_at).whole_seconds();
            if age > max_age_seconds {
                State::Stale
            } else if let Some(prev) = last_generation_id {
                if prev == a.generation_id && same_gen_run >= stuck_after_polls {
                    State::Stuck
                } else {
                    State::Healthy
                }
            } else {
                State::Healthy
            }
        }
    }
}

pub async fn run(cmd: SentinelCmd) -> anyhow::Result<()> {
    let config_text = std::fs::read_to_string(&cmd.config)?;
    let config: SentinelConfig = serde_json::from_str(&config_text)?;

    let artifact_path = PathBuf::from(&config.artifact_path);
    let poll_interval = Duration::from_secs(config.poll_interval_seconds);
    let grace_period = Duration::from_secs(config.grace_period_seconds);
    let started_at = std::time::Instant::now();

    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    info!(
        artifact = %config.artifact_path,
        max_age_s = config.max_age_seconds,
        poll_s = config.poll_interval_seconds,
        channels = config.channels.len(),
        "sentinel starting"
    );

    let mut last_state: State = State::Starting;
    let mut last_generation_id: Option<i64> = None;
    let mut same_gen_run: u64 = 0;

    loop {
        let artifact = read_liveness(&artifact_path);
        let now = OffsetDateTime::now_utc();

        // In grace period, don't alert on missing/malformed; wait for the
        // first successful read or for the grace period to elapse.
        let in_grace = started_at.elapsed() < grace_period
            && matches!(artifact, Err(LivenessReadError::Missing));

        // Track same-generation run count for stuck detection
        if let Ok(ref a) = artifact {
            if last_generation_id == Some(a.generation_id) {
                same_gen_run += 1;
            } else {
                same_gen_run = 0;
            }
            last_generation_id = Some(a.generation_id);
        }

        let state = if in_grace {
            State::Starting
        } else {
            classify(
                now,
                config.max_age_seconds,
                &artifact,
                last_generation_id,
                same_gen_run,
                config.stuck_after_polls,
            )
        };

        // Alert on state transition only (deduplication)
        if state != last_state {
            match (last_state, state) {
                (_, State::Starting) => { /* no alert */ }
                (State::Starting, State::Healthy) => {
                    info!("sentinel: initial healthy read");
                }
                (prev, State::Healthy) if prev != State::Starting => {
                    let msg = format!(
                        "🟢 NQ sentinel: **recovered** from {} — generation #{} at {}",
                        prev.as_str(),
                        last_generation_id.map(|g| g.to_string()).unwrap_or_else(|| "?".into()),
                        artifact.as_ref().ok().map(|a| a.generated_at.as_str()).unwrap_or("?"),
                    );
                    send_alert(&http_client, &config.channels, &msg).await;
                }
                (_, new) => {
                    let detail = match &artifact {
                        Ok(a) => format!("last generation #{} at {}", a.generation_id, a.generated_at),
                        Err(e) => format!("read error: {}", e),
                    };
                    let msg = format!(
                        "🔴 NQ sentinel: **{}** — {}",
                        new.as_str(),
                        detail,
                    );
                    send_alert(&http_client, &config.channels, &msg).await;
                }
            }
            last_state = state;
        }

        tokio::time::sleep(poll_interval).await;
    }
}

async fn send_alert(client: &reqwest::Client, channels: &[NotificationChannel], msg: &str) {
    for channel in channels {
        let result = match channel {
            NotificationChannel::Discord { webhook_url } => {
                client.post(webhook_url)
                    .json(&serde_json::json!({ "content": msg }))
                    .send().await
            }
            NotificationChannel::Slack { webhook_url } => {
                client.post(webhook_url)
                    .json(&serde_json::json!({ "text": msg }))
                    .send().await
            }
            NotificationChannel::Webhook { url, headers } => {
                let mut req = client.post(url)
                    .json(&serde_json::json!({ "text": msg }));
                for (k, v) in headers {
                    req = req.header(k, v);
                }
                req.send().await
            }
        };
        match result {
            Ok(r) if r.status().is_success() => {
                info!("sentinel alert sent: {}", msg);
            }
            Ok(r) => {
                warn!(status = %r.status(), "sentinel alert non-2xx");
            }
            Err(e) => {
                error!(err = %e, "sentinel alert send failed");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nq_db::{write_liveness, CURRENT_SCHEMA_VERSION, LIVENESS_FORMAT_VERSION};
    use tempfile::tempdir;

    fn art(gen: i64, generated_at: &str) -> LivenessArtifact {
        LivenessArtifact {
            liveness_format_version: LIVENESS_FORMAT_VERSION,
            instance_id: Some("test".into()),
            generated_at: generated_at.into(),
            generation_id: gen,
            schema_version: CURRENT_SCHEMA_VERSION,
            contract_version: None,
            build_commit: None,
            findings_observed: 0,
            findings_suppressed: 0,
            detectors_run: 0,
            status: "ok".into(),
        }
    }

    #[test]
    fn healthy_fresh_artifact() {
        let now = OffsetDateTime::parse("2026-04-14T12:00:00Z", &Rfc3339).unwrap();
        let a = Ok(art(42, "2026-04-14T11:59:00Z")); // 60s old
        let state = classify(now, 180, &a, None, 0, 5);
        assert_eq!(state, State::Healthy);
    }

    #[test]
    fn stale_when_timestamp_exceeds_max_age() {
        let now = OffsetDateTime::parse("2026-04-14T12:00:00Z", &Rfc3339).unwrap();
        let a = Ok(art(42, "2026-04-14T11:55:00Z")); // 300s old
        let state = classify(now, 180, &a, None, 0, 5);
        assert_eq!(state, State::Stale);
    }

    #[test]
    fn missing_when_file_absent() {
        let now = OffsetDateTime::now_utc();
        let a: Result<LivenessArtifact, LivenessReadError> = Err(LivenessReadError::Missing);
        assert_eq!(classify(now, 180, &a, None, 0, 5), State::Missing);
    }

    #[test]
    fn malformed_on_parse_error() {
        let now = OffsetDateTime::now_utc();
        let err = serde_json::from_str::<LivenessArtifact>("nope").unwrap_err();
        let a = Err(LivenessReadError::Malformed(err));
        assert_eq!(classify(now, 180, &a, None, 0, 5), State::Malformed);
    }

    #[test]
    fn malformed_timestamp_is_malformed() {
        let now = OffsetDateTime::now_utc();
        let a = Ok(art(42, "not-a-timestamp"));
        assert_eq!(classify(now, 180, &a, None, 0, 5), State::Malformed);
    }

    #[test]
    fn stuck_when_generation_id_unchanged_for_threshold_polls() {
        let now = OffsetDateTime::parse("2026-04-14T12:00:00Z", &Rfc3339).unwrap();
        let a = Ok(art(42, "2026-04-14T11:59:30Z")); // fresh
        // same_gen_run=5 >= stuck_after_polls=5 → stuck
        let state = classify(now, 180, &a, Some(42), 5, 5);
        assert_eq!(state, State::Stuck);
    }

    #[test]
    fn not_stuck_below_threshold() {
        let now = OffsetDateTime::parse("2026-04-14T12:00:00Z", &Rfc3339).unwrap();
        let a = Ok(art(42, "2026-04-14T11:59:30Z"));
        let state = classify(now, 180, &a, Some(42), 3, 5);
        assert_eq!(state, State::Healthy);
    }

    #[test]
    fn real_file_round_trip_classify() {
        // End-to-end: write an artifact, read it back, classify.
        let dir = tempdir().unwrap();
        let path = dir.path().join("liveness.json");
        write_liveness(&path, &art(100, "2026-04-14T12:00:00Z")).unwrap();
        let read = read_liveness(&path);
        assert!(read.is_ok());
        let now = OffsetDateTime::parse("2026-04-14T12:01:00Z", &Rfc3339).unwrap();
        let state = classify(now, 180, &read, None, 0, 5);
        assert_eq!(state, State::Healthy);
    }
}
