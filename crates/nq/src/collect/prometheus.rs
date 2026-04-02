//! Prometheus exposition format scraper.
//!
//! Scrapes configured /metrics endpoints and parses the Prometheus text format
//! into MetricSample values. No external parser crate — the format is simple
//! enough to parse in ~100 lines.

use nq_core::wire::{CollectorPayload, MetricSample};
use nq_core::{CollectorStatus, PublisherConfig};
use std::collections::BTreeMap;
use time::OffsetDateTime;

pub fn collect(config: &PublisherConfig) -> CollectorPayload<Vec<MetricSample>> {
    let now = OffsetDateTime::now_utc();

    if config.prometheus_targets.is_empty() {
        return CollectorPayload {
            status: CollectorStatus::Skipped,
            collected_at: Some(now),
            error_message: None,
            data: None,
        };
    }

    let mut all_samples = Vec::new();
    let mut errors = Vec::new();

    for target in &config.prometheus_targets {
        match scrape_target(target) {
            Ok(samples) => all_samples.extend(samples),
            Err(e) => errors.push(format!("{}: {}", target.name, e)),
        }
    }

    if all_samples.is_empty() && !errors.is_empty() {
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
        error_message: if errors.is_empty() {
            None
        } else {
            Some(errors.join("; "))
        },
        data: Some(all_samples),
    }
}

fn scrape_target(target: &nq_core::config::PrometheusTarget) -> anyhow::Result<Vec<MetricSample>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_millis(target.timeout_ms))
        .build()?;

    let body = client.get(&target.url).send()?.text()?;
    Ok(parse_exposition(&body))
}

/// Parse Prometheus exposition format text into metric samples.
///
/// Handles:
/// - # HELP lines (ignored)
/// - # TYPE lines (captures metric type)
/// - metric_name{label="value",...} value [timestamp]
/// - metric_name value [timestamp]
fn parse_exposition(text: &str) -> Vec<MetricSample> {
    let mut samples = Vec::new();
    let mut current_type: Option<String> = None;
    let mut current_type_name: Option<String> = None;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("# HELP") {
            continue;
        }
        if line.starts_with("# TYPE") {
            // # TYPE metric_name type
            let parts: Vec<&str> = line.splitn(4, ' ').collect();
            if parts.len() >= 4 {
                current_type_name = Some(parts[2].to_string());
                current_type = Some(parts[3].to_string());
            }
            continue;
        }
        if line.starts_with('#') {
            continue;
        }

        if let Some(sample) = parse_sample_line(line, &current_type_name, &current_type) {
            samples.push(sample);
        }
    }

    samples
}

fn parse_sample_line(
    line: &str,
    type_name: &Option<String>,
    metric_type: &Option<String>,
) -> Option<MetricSample> {
    // Split into name, labels, and value string
    let (name, labels, rest) = if let Some(brace_start) = line.find('{') {
        let brace_end = line.find('}')?;
        let name = &line[..brace_start];
        let labels_str = &line[brace_start + 1..brace_end];
        let rest = line[brace_end + 1..].trim();
        let labels = parse_labels(labels_str);
        (name, labels, rest)
    } else {
        // No labels — split on first space
        let mut parts = line.splitn(2, ' ');
        let name = parts.next()?;
        let rest = parts.next()?.trim();
        (name, BTreeMap::new(), rest)
    };

    // Value is the first token in rest (ignore optional timestamp)
    let value_str: &str = rest.split_whitespace().next()?;
    let value = match value_str {
        "+Inf" => f64::INFINITY,
        "-Inf" => f64::NEG_INFINITY,
        "NaN" => f64::NAN,
        s => s.parse::<f64>().ok()?,
    };

    // Determine metric_type: use the TYPE declaration if the base name matches
    let mt = metric_type
        .as_ref()
        .filter(|_| {
            type_name
                .as_ref()
                .map(|tn| name.starts_with(tn.as_str()))
                .unwrap_or(false)
        })
        .cloned();

    Some(MetricSample {
        name: name.to_string(),
        labels,
        value,
        metric_type: mt,
    })
}

fn parse_labels(labels_str: &str) -> BTreeMap<String, String> {
    let mut labels = BTreeMap::new();
    if labels_str.is_empty() {
        return labels;
    }

    // Simple state machine for label="value" pairs
    let mut remaining = labels_str;
    loop {
        remaining = remaining.trim_start_matches([',', ' ']);
        if remaining.is_empty() {
            break;
        }

        let eq = match remaining.find('=') {
            Some(i) => i,
            None => break,
        };
        let key = remaining[..eq].trim();
        remaining = &remaining[eq + 1..];

        // Value is quoted
        if remaining.starts_with('"') {
            remaining = &remaining[1..];
            let mut value = String::new();
            let mut chars = remaining.chars();
            loop {
                match chars.next() {
                    Some('\\') => {
                        if let Some(c) = chars.next() {
                            match c {
                                'n' => value.push('\n'),
                                '\\' => value.push('\\'),
                                '"' => value.push('"'),
                                other => {
                                    value.push('\\');
                                    value.push(other);
                                }
                            }
                        }
                    }
                    Some('"') => break,
                    Some(c) => value.push(c),
                    None => break,
                }
            }
            remaining = chars.as_str();
            labels.insert(key.to_string(), value);
        }
    }

    labels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_gauge() {
        let text = r#"# HELP node_load1 1m load average.
# TYPE node_load1 gauge
node_load1 0.72
"#;
        let samples = parse_exposition(text);
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].name, "node_load1");
        assert!((samples[0].value - 0.72).abs() < f64::EPSILON);
        assert_eq!(samples[0].metric_type.as_deref(), Some("gauge"));
        assert!(samples[0].labels.is_empty());
    }

    #[test]
    fn parse_with_labels() {
        let text = r#"# TYPE http_requests_total counter
http_requests_total{method="GET",code="200"} 1234
http_requests_total{method="POST",code="500"} 5
"#;
        let samples = parse_exposition(text);
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].labels["method"], "GET");
        assert_eq!(samples[0].labels["code"], "200");
        assert!((samples[0].value - 1234.0).abs() < f64::EPSILON);
        assert_eq!(samples[1].labels["method"], "POST");
    }

    #[test]
    fn parse_histogram_bucket() {
        let text = r#"# TYPE http_duration_seconds histogram
http_duration_seconds_bucket{le="0.1"} 24054
http_duration_seconds_bucket{le="+Inf"} 144320
http_duration_seconds_sum 53423
http_duration_seconds_count 144320
"#;
        let samples = parse_exposition(text);
        assert_eq!(samples.len(), 4);
        assert!((samples[0].value - 24054.0).abs() < f64::EPSILON);
        assert!((samples[1].value - 144320.0).abs() < f64::EPSILON);
        assert_eq!(samples[1].labels["le"], "+Inf");
    }

    #[test]
    fn parse_nan_value() {
        let samples = parse_exposition("some_metric NaN\n");
        assert_eq!(samples.len(), 1);
        assert!(samples[0].value.is_nan());
    }

    #[test]
    fn parse_escaped_label() {
        let text = r#"metric{path="/foo\"bar"} 1.0"#;
        let samples = parse_exposition(text);
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].labels["path"], "/foo\"bar");
    }
}
