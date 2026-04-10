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
    // Try to parse a 20-byte RFC 3339 timestamp at various offsets in the line.
    // Slice by character boundaries (not byte indices) to avoid panics on
    // multi-byte UTF-8 lines, and bound the end index to line.len() so we
    // never overshoot.
    let bytes = line.as_bytes();
    if bytes.len() < 20 {
        return None;
    }
    let max_start = bytes.len().saturating_sub(20).min(40);
    for start in 0..=max_start {
        let end = start + 20;
        // Skip offsets that don't land on UTF-8 char boundaries.
        if !line.is_char_boundary(start) || !line.is_char_boundary(end) {
            continue;
        }
        if let Ok(ts) = OffsetDateTime::parse(&line[start..end], &Rfc3339) {
            return Some(ts);
        }
    }
    None
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    // Find the largest char boundary <= max so we never slice mid-codepoint.
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &s[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_timestamp_real_log_line_does_not_panic() {
        // The actual log line that took down nq-publish on 2026-04-09:
        // byte index 59 was out of bounds. Repro and assert no panic.
        let line = "2026-04-09 18:08:47,657 INFO labelwatch.runner rss=290.3MB";
        let _ = extract_timestamp(line); // must not panic; result is fine if None
    }

    #[test]
    fn extract_timestamp_short_line() {
        // Lines shorter than 20 bytes must be handled cleanly.
        assert_eq!(extract_timestamp(""), None);
        assert_eq!(extract_timestamp("hi"), None);
        assert_eq!(extract_timestamp("19 chars exactly!!!"), None);
    }

    #[test]
    fn extract_timestamp_finds_rfc3339_at_start() {
        let line = "2026-04-09T18:08:47Z some message";
        let ts = extract_timestamp(line);
        assert!(ts.is_some(), "should find RFC3339 timestamp at start of line");
    }

    #[test]
    fn extract_timestamp_handles_multibyte_utf8() {
        // Multi-byte UTF-8 characters in the line must not cause a slice
        // panic at non-char-boundary byte indices.
        let line = "日本語 log line with unicode characters and no timestamp";
        let _ = extract_timestamp(line); // must not panic
    }

    #[test]
    fn truncate_handles_multibyte_utf8() {
        // Truncating mid-codepoint must round down to a char boundary,
        // not panic with "byte index N is not a char boundary".
        let s = "日本語のログ"; // 6 chars, 18 bytes
        let result = truncate(s, 5);
        assert!(result.ends_with("..."));
        // Whatever it returns must be valid UTF-8 (no panic)
        assert!(result.len() <= s.len() + 3);
    }

    #[test]
    fn truncate_short_string_unchanged() {
        assert_eq!(truncate("hello", 100), "hello");
    }

    #[test]
    fn truncate_ascii_at_boundary() {
        assert_eq!(truncate("hello world", 5), "hello...");
    }
}
