//! Log collector: fetch bounded windows from journald or files,
//! reduce to observations + exemplars. No raw storage — classified
//! counts only.

use nq_core::wire::{CollectorPayload, LogExample, LogObservation};
use nq_core::{CollectorStatus, Platform, PublisherConfig};
use std::process::Command;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

pub fn collect(config: &PublisherConfig) -> CollectorPayload<Vec<LogObservation>> {
    collect_for(config, Platform::current())
}

/// Log collection, dispatched per source so the platform-specific path
/// is testable on Linux CI:
/// - the cross-platform **file** adapter runs on every platform;
/// - the **journald** adapter is Linux-only — on a non-Linux platform
///   that source emits a `fetch_status: "not_supported"` observation
///   (typed per-source incapacity: never a fabricated count, never a
///   silent drop, never shelling a missing `journalctl`).
///
/// This removes the Slice-0 over-refusal that returned whole-collector
/// `not_supported` on non-Linux even for portable file sources. Linux
/// behavior is unchanged. (A native macOS unified-logging witness is a
/// separate, schema-bearing slice.)
pub fn collect_for(
    config: &PublisherConfig,
    platform: Platform,
) -> CollectorPayload<Vec<LogObservation>> {
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
        // journald is Linux-only. On a non-Linux substrate the source is
        // structurally unobservable here — emit a typed per-source
        // not_supported observation rather than dropping it (green
        // silence) or shelling a missing journalctl (generic error).
        if src.adapter == "journald" && platform != Platform::Linux {
            observations.push(not_supported_observation(&src.source_id, now));
            continue;
        }
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

/// A typed per-source refusal: the source is structurally unobservable
/// on this platform (e.g. a journald source on non-Linux). Zero counts,
/// no exemplars, `fetch_status = "not_supported"` — distinct from
/// `source_quiet` (observed, nothing in window) and from a fetch error.
fn not_supported_observation(source_id: &str, now: OffsetDateTime) -> LogObservation {
    LogObservation {
        source_id: source_id.to_string(),
        window_start: now - time::Duration::seconds(60),
        window_end: now,
        fetch_status: "not_supported".to_string(),
        lines_total: 0,
        lines_error: 0,
        lines_warn: 0,
        last_log_ts: None,
        transport_lag_ms: None,
        examples: Vec::new(),
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

#[cfg(test)]
mod platform_tests {
    use super::*;
    use nq_core::config::LogSourceConfig;
    use std::io::Write;

    fn src(source_id: &str, adapter: &str, target: &str) -> LogSourceConfig {
        LogSourceConfig {
            source_id: source_id.into(),
            adapter: adapter.into(),
            target: target.into(),
            silence_budget_secs: 120,
            max_lines: 100,
        }
    }

    fn cfg_with(sources: Vec<LogSourceConfig>) -> PublisherConfig {
        PublisherConfig {
            log_sources: sources,
            ..PublisherConfig::default()
        }
    }

    #[test]
    fn empty_config_is_skipped_on_any_platform() {
        for plat in [Platform::Linux, Platform::MacOs, Platform::FreeBsd, Platform::Other] {
            let p = collect_for(&PublisherConfig::default(), plat);
            assert_eq!(p.status, CollectorStatus::Skipped, "{plat:?}");
            assert_ne!(p.status, CollectorStatus::NotSupported);
        }
    }

    #[test]
    fn file_source_runs_on_non_linux_not_blocked() {
        // The file adapter is cross-platform: on a non-Linux substrate it
        // must RUN, not be blocked by a whole-collector platform gate.
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(f, "line one\nline two").unwrap();
        let path = f.path().to_str().unwrap().to_string();
        let p = collect_for(&cfg_with(vec![src("applog", "file", &path)]), Platform::MacOs);
        assert_eq!(p.status, CollectorStatus::Ok);
        let obs = p.data.expect("observations");
        assert_eq!(obs.len(), 1);
        assert_ne!(obs[0].fetch_status, "not_supported", "file source is observable on macOS");
    }

    #[test]
    fn journald_source_on_non_linux_is_not_supported_per_source() {
        // journald is Linux-only: on a non-Linux substrate the source is
        // marked not_supported per-source (typed, not green silence, not
        // a generic error), and the collector still reports Ok.
        let p = collect_for(&cfg_with(vec![src("syslog", "journald", "sshd")]), Platform::FreeBsd);
        assert_eq!(p.status, CollectorStatus::Ok);
        let obs = p.data.expect("observations");
        assert_eq!(obs.len(), 1);
        assert_eq!(obs[0].fetch_status, "not_supported");
        assert_eq!(obs[0].lines_total, 0);
        assert!(obs[0].examples.is_empty());
    }

    #[test]
    fn linux_path_unchanged() {
        // Empty config → Skipped on Linux, as before.
        assert_eq!(
            collect_for(&PublisherConfig::default(), Platform::Linux).status,
            CollectorStatus::Skipped
        );
    }
}
