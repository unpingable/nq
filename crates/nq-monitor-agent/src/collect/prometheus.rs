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
    Ok(stamp_with_target(parse_exposition(&body), target))
}

/// Stamp parsed samples with their scrape-target provenance. Called
/// after `parse_exposition` so parsing itself stays pure; the function
/// that knows which target the body came from is the one that records
/// it on each sample. See `MetricSample` docs for the discipline (no
/// `nq_*` label injection — provenance lives on the struct, outside
/// the exporter's label namespace).
fn stamp_with_target(
    mut samples: Vec<MetricSample>,
    target: &nq_core::config::PrometheusTarget,
) -> Vec<MetricSample> {
    for sample in &mut samples {
        sample.scrape_target_name = Some(target.name.clone());
        sample.scrape_target_url = Some(target.url.clone());
    }
    samples
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
        // Parsing stays pure — provenance is stamped by
        // `stamp_with_target` after parse completes.
        scrape_target_name: None,
        scrape_target_url: None,
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

    #[test]
    fn samples_from_two_targets_remain_distinguishable() {
        // Two scrape targets that both emit `probe_success` from a
        // single blackbox exporter — different `target=` query
        // parameters but identical metric name and labels. Without
        // target-provenance stamping, the parsed samples would be
        // indistinguishable in storage. With it, each sample carries
        // its source target's name and URL.
        let target_a = nq_core::config::PrometheusTarget {
            name: "blackbox_labelwatch_health".to_string(),
            url: "http://127.0.0.1:9115/probe?module=http_2xx&target=https://labelwatch/health"
                .to_string(),
            timeout_ms: 5000,
        };
        let target_b = nq_core::config::PrometheusTarget {
            name: "blackbox_neutralzone_home".to_string(),
            url: "http://127.0.0.1:9115/probe?module=http_2xx&target=https://nq.neutral.zone/"
                .to_string(),
            timeout_ms: 5000,
        };
        let body = "# TYPE probe_success gauge\nprobe_success 1\n";

        let samples_a = stamp_with_target(parse_exposition(body), &target_a);
        let samples_b = stamp_with_target(parse_exposition(body), &target_b);

        assert_eq!(samples_a.len(), 1);
        assert_eq!(samples_b.len(), 1);

        // Each sample names its source target.
        assert_eq!(
            samples_a[0].scrape_target_name.as_deref(),
            Some("blackbox_labelwatch_health")
        );
        assert_eq!(
            samples_b[0].scrape_target_name.as_deref(),
            Some("blackbox_neutralzone_home")
        );

        // The two samples are now distinguishable in storage even
        // though metric name and labels are identical.
        assert_ne!(samples_a[0].scrape_target_name, samples_b[0].scrape_target_name);
        assert_ne!(samples_a[0].scrape_target_url, samples_b[0].scrape_target_url);

        // Exporter-emitted content stays untouched — no nq_* label
        // injection, no clobbering. Both samples still carry the same
        // metric name and (here empty) label set.
        assert_eq!(samples_a[0].name, samples_b[0].name);
        assert_eq!(samples_a[0].labels, samples_b[0].labels);
        assert_eq!(samples_a[0].value, samples_b[0].value);
    }

    #[test]
    fn parsed_samples_without_stamping_have_none_provenance() {
        // parse_exposition stays pure — it doesn't know about targets.
        // Provenance is None until stamp_with_target sets it. This pins
        // the invariant that parsing and stamping are separate stages.
        let body = "# TYPE probe_success gauge\nprobe_success 1\n";
        let samples = parse_exposition(body);
        assert_eq!(samples.len(), 1);
        assert!(samples[0].scrape_target_name.is_none());
        assert!(samples[0].scrape_target_url.is_none());
    }

    #[test]
    fn provenance_fields_skip_serialization_when_none() {
        // Additive-and-optional contract: an unstamped sample
        // (legacy-shaped payload) must serialize without the new
        // provenance keys at all, so older readers see no change.
        let sample = MetricSample {
            name: "probe_success".to_string(),
            labels: BTreeMap::new(),
            value: 1.0,
            metric_type: Some("gauge".to_string()),
            scrape_target_name: None,
            scrape_target_url: None,
        };
        let json = serde_json::to_string(&sample).unwrap();
        assert!(
            !json.contains("scrape_target_name"),
            "unstamped sample must omit scrape_target_name from JSON: {json}"
        );
        assert!(
            !json.contains("scrape_target_url"),
            "unstamped sample must omit scrape_target_url from JSON: {json}"
        );
    }
}
