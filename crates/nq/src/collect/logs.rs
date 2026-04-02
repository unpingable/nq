//! Log collector: fetch bounded windows from journald or files,
//! reduce to observations + exemplars. No raw storage — classified
//! counts only.

use nq_core::wire::{CollectorPayload, LogExample, LogObservation};
use nq_core::{CollectorStatus, PublisherConfig};
use std::process::Command;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

pub fn collect(config: &PublisherConfig) -> CollectorPayload<Vec<LogObservation>> {
    let now = OffsetDateTime::now_utc();

    if config.log_sources.is_empty() {
        return CollectorPayload {
            status: CollectorStatus::Skipped,
            collected_at: Some(now),
            error_message: None,
            data: None,
        };
    }

    let mut observations = Vec::new();
    let mut errors = Vec::new();

    for src in &config.log_sources {
        match collect_source(src, now) {
            Ok(obs) => observations.push(obs),
            Err(e) => errors.push(format!("{}: {}", src.source_id, e)),
        }
    }

    if observations.is_empty() && !errors.is_empty() {
        return CollectorPayload {
            status: CollectorStatus::Error,
            collected_at: Some(now),
            error_message: Some(errors.join("; ")),
            data: None,
        };
    }

    CollectorPayload {
        status: CollectorStatus::Ok,
        collected_at: Some(now),
        error_message: if errors.is_empty() { None } else { Some(errors.join("; ")) },
        data: Some(observations),
    }
}

fn collect_source(
    src: &nq_core::config::LogSourceConfig,
    now: OffsetDateTime,
) -> anyhow::Result<LogObservation> {
    let window_secs = 60; // one generation window
    let window_start = now - time::Duration::seconds(window_secs);

    let lines = match src.adapter.as_str() {
        "journald" => fetch_journald(&src.target, window_secs, src.max_lines)?,
        "file" => fetch_file_tail(&src.target, src.max_lines)?,
        other => anyhow::bail!("unknown adapter: {}", other),
    };

    // Classify lines
    let mut total: u64 = 0;
    let mut errors: u64 = 0;
    let mut warns: u64 = 0;
    let mut last_ts: Option<OffsetDateTime> = None;
    let mut error_examples: Vec<LogExample> = Vec::new();

    for line in &lines {
        total += 1;
        let sev = classify_severity(line);

        if sev == "error" || sev == "fatal" {
            errors += 1;
            if error_examples.len() < 5 {
                error_examples.push(LogExample {
                    ts: Some(now),
                    severity: sev.to_string(),
                    message: truncate(line, 200),
                });
            }
        } else if sev == "warn" {
            warns += 1;
        }
    }

    // Try to extract timestamp from last line
    if let Some(last_line) = lines.last() {
        last_ts = extract_timestamp(last_line);
    }

    let fetch_status = if lines.is_empty() && total == 0 {
        "source_quiet"
    } else {
        "ok"
    };

    Ok(LogObservation {
        source_id: src.source_id.clone(),
        window_start,
        window_end: now,
        fetch_status: fetch_status.to_string(),
        lines_total: total,
        lines_error: errors,
        lines_warn: warns,
        last_log_ts: last_ts,
        transport_lag_ms: None,
        examples: error_examples,
    })
}

fn fetch_journald(unit: &str, window_secs: i64, max_lines: usize) -> anyhow::Result<Vec<String>> {
    let output = Command::new("journalctl")
        .args([
            "-u", unit,
            "--since", &format!("{} seconds ago", window_secs),
            "--no-pager",
            "-o", "cat",
            "-n", &max_lines.to_string(),
        ])
        .output()?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("journalctl failed: {}", err.trim());
    }

    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text.lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect())
}

fn fetch_file_tail(path: &str, max_lines: usize) -> anyhow::Result<Vec<String>> {
    let output = Command::new("tail")
        .args(["-n", &max_lines.to_string(), path])
        .output()?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("tail failed: {}", err.trim());
    }

    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text.lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect())
}

fn classify_severity(line: &str) -> &'static str {
    let lower = line.to_lowercase();
    if lower.contains("fatal") || lower.contains("panic") || lower.contains("critical") {
        "fatal"
    } else if lower.contains("error") || lower.contains("exception") || lower.contains("fail") {
        "error"
    } else if lower.contains("warn") {
        "warn"
    } else {
        "info"
    }
}

fn extract_timestamp(line: &str) -> Option<OffsetDateTime> {
    // Try ISO 8601 / RFC 3339 first
    if line.len() >= 20 {
        for start in 0..line.len().min(40) {
            if let Ok(ts) = OffsetDateTime::parse(&line[start..start.min(line.len()).max(start + 20)], &Rfc3339) {
                return Some(ts);
            }
        }
    }
    None
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}
