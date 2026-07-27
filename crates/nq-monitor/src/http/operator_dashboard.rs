//! Task-first dashboard rendering.
//!
//! The database module owns snapshot coherence and finding identity. This
//! module only translates that bounded state into an operator-facing decision,
//! evidence, and expert-detail hierarchy.

use nq_db::dashboard::{
    DashboardBasis, DashboardCurrentFindingDetail, DashboardEvidence, DashboardFinding,
    DashboardFindingDetail, DashboardFindingStatus, DashboardHistoricalFindingDetail,
    DashboardInventory, DashboardMissingFindingDetail, DashboardOverview,
};
use nq_db::finding_actions::{FindingAction, FindingActionContract, TtlPolicy};

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

fn url_encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn human_duration(seconds: i64) -> String {
    if seconds < 0 {
        return "clock difference unknown".into();
    }
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3_600 {
        format!("{}m", seconds / 60)
    } else if seconds < 86_400 {
        format!("{}h", seconds / 3_600)
    } else {
        format!("{}d", seconds / 86_400)
    }
}

fn optional_text(value: Option<&str>) -> String {
    value
        .filter(|value| !value.is_empty())
        .map(escape_html)
        .unwrap_or_else(|| "<span class=\"unknown-value\">Unavailable</span>".into())
}

fn optional_number<T: std::fmt::Display>(value: Option<T>, suffix: &str) -> String {
    value
        .map(|value| format!("{}{suffix}", escape_html(&value.to_string())))
        .unwrap_or_else(|| "<span class=\"unknown-value\">Unavailable</span>".into())
}

fn plain_domain(domain: &str) -> &'static str {
    match domain {
        "Δo" => "Observation is missing",
        "Δs" => "Signal quality changed",
        "Δg" => "Supporting substrate is under pressure",
        "Δh" => "A condition is worsening over time",
        "component_testimony" => "NQ's observation path is incomplete",
        "signal" => "Signal quality changed",
        "substrate" => "Supporting substrate is under pressure",
        "meta" => "NQ's own observation path changed",
        _ => "Internal classification is not translated",
    }
}

fn response_label(action_bias: Option<&str>) -> &'static str {
    match action_bias.map(normalize).as_deref() {
        Some("intervenenow") => "Review for immediate intervention",
        Some("intervenesoon") => "Plan an intervention soon",
        Some("investigatenow") => "Investigate now",
        Some("investigatebusinesshours") => "Investigate during working hours",
        Some("watch") => "Watch",
        _ => "Response not established",
    }
}

fn status_label(status: DashboardFindingStatus) -> &'static str {
    match status {
        DashboardFindingStatus::Ongoing => "Ongoing",
        DashboardFindingStatus::Recovering => "No longer observed; confirming",
        DashboardFindingStatus::Stale => "Stale evidence",
        DashboardFindingStatus::Suppressed => "Observation unavailable",
        DashboardFindingStatus::Retired => "Historical evidence",
    }
}

fn status_class(status: DashboardFindingStatus) -> &'static str {
    match status {
        DashboardFindingStatus::Ongoing => "state-ongoing",
        DashboardFindingStatus::Recovering => "state-recovering",
        DashboardFindingStatus::Stale => "state-stale",
        DashboardFindingStatus::Suppressed => "state-unknown",
        DashboardFindingStatus::Retired => "state-historical",
    }
}

fn is_muted_work_state(work_state: &str) -> bool {
    matches!(
        normalize(work_state).as_str(),
        "quiesced" | "closed" | "suppressed"
    )
}

fn needs_attention(finding: &DashboardFinding) -> bool {
    finding.status == DashboardFindingStatus::Ongoing
        && !is_muted_work_state(&finding.work_state)
        && normalize(finding.diagnosis.action_bias.as_deref().unwrap_or_default()) != "watch"
}

fn is_unknown_state(finding: &DashboardFinding) -> bool {
    matches!(
        finding.status,
        DashboardFindingStatus::Recovering
            | DashboardFindingStatus::Stale
            | DashboardFindingStatus::Suppressed
            | DashboardFindingStatus::Retired
    )
}

fn is_recent_change(finding: &DashboardFinding) -> bool {
    matches!(
        finding.stability.as_deref().map(normalize).as_deref(),
        Some("new") | Some("flickering") | Some("recovering")
    )
}

fn finding_title(finding: &DashboardFinding) -> String {
    let current = finding.status == DashboardFindingStatus::Ongoing;
    match finding.kind.as_str() {
        "error_shift" if !finding.subject.is_empty() => {
            return if current {
                format!("{} error rate increased", escape_html(&finding.subject))
            } else {
                format!(
                    "{} had an error-rate increase in the last observation",
                    escape_html(&finding.subject)
                )
            };
        }
        "disk_pressure" if !finding.host.is_empty() => {
            return if current {
                format!("{} disk is nearing capacity", escape_html(&finding.host))
            } else {
                format!(
                    "{} disk pressure was observed previously",
                    escape_html(&finding.host)
                )
            };
        }
        "freelist_bloat" if !finding.subject.is_empty() => {
            return if current {
                format!(
                    "{} has reclaimable database space",
                    escape_html(&finding.subject)
                )
            } else {
                format!(
                    "{} previously had reclaimable database space",
                    escape_html(&finding.subject)
                )
            };
        }
        "smart_status_lies" if !finding.subject.is_empty() => {
            return if current {
                format!(
                    "{} SMART status conflicts with error counters",
                    escape_html(&finding.subject)
                )
            } else {
                format!(
                    "{} SMART sources disagreed in the last observation",
                    escape_html(&finding.subject)
                )
            };
        }
        _ => {}
    }

    if let Some(synopsis) = finding
        .diagnosis
        .synopsis
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        return escape_html(synopsis);
    }

    let meta = nq_db::finding_meta::finding_meta(&finding.kind);
    if finding.subject.is_empty() {
        escape_html(meta.plain_label)
    } else {
        format!(
            "{} — {}",
            escape_html(meta.plain_label),
            escape_html(&finding.subject)
        )
    }
}

fn finding_href(finding_key: &str) -> String {
    format!("/finding?key={}", url_encode(finding_key))
}

fn impact_statement(finding: &DashboardFinding) -> String {
    let impact = finding
        .diagnosis
        .service_impact
        .as_deref()
        .map(normalize)
        .unwrap_or_default();
    match impact.as_str() {
        "nonecurrent" | "none" => {
            "No current service impact is recorded. That is not proof of no impact.".into()
        }
        "degraded" if finding.kind == "error_shift" => {
            "Error output is degraded; user impact has not been independently established.".into()
        }
        "degraded" => "NQ records a currently degraded operational signal.".into(),
        "immediaterisk" => "The detector records immediate operational risk.".into(),
        _ => "Current operational impact is unknown.".into(),
    }
}

fn unknowns(finding: &DashboardFinding) -> Vec<String> {
    let mut items = vec!["Cause is not established by this finding.".to_string()];
    if finding.kind == "error_shift" {
        items.push("User-visible impact is not independently established.".into());
    }
    if normalize(&finding.basis_state) == "unknown" {
        items.push("The finding does not carry a complete evidence-basis identity.".into());
    }
    match finding.status {
        DashboardFindingStatus::Recovering => items.push(
            "The condition stopped appearing, but NQ is still inside its confirmation window."
                .into(),
        ),
        DashboardFindingStatus::Stale => {
            items.push("The last observation is too old to describe current state.".into())
        }
        DashboardFindingStatus::Suppressed => items.push(
            "Observation is unavailable; last-known state must not be read as current.".into(),
        ),
        DashboardFindingStatus::Retired => items.push(
            "The evidence source was deliberately retired; current state is not established."
                .into(),
        ),
        DashboardFindingStatus::Ongoing => {}
    }
    items.sort();
    items.dedup();
    items
}

fn render_basis(basis: &DashboardBasis) -> String {
    match (
        basis.generation_id,
        basis.completed_at.as_deref(),
        basis.age_seconds,
        basis.status.as_deref(),
    ) {
        (Some(generation), Some(observed_at), Some(age), Some(status)) => format!(
            "<div class=\"snapshot-basis\" data-generation=\"{generation}\">\
               <span class=\"basis-label\">Observation basis</span>\
               <time datetime=\"{at}\">{at}</time>\
               <span>({age} ago)</span>\
               <span>publish status: {status}</span>\
               <span class=\"expert-token\" title=\"A snapshot is one atomic NQ publish unit; it makes values on this page comparable.\">snapshot #{generation}</span>\
             </div>",
            generation = generation,
            at = escape_html(observed_at),
            age = escape_html(&human_duration(age)),
            status = escape_html(status),
        ),
        _ => "<div class=\"snapshot-basis state-unknown\"><span class=\"basis-label\">Observation basis unavailable</span></div>".into(),
    }
}

fn render_error_shift_summary(evidence: &nq_db::dashboard::ErrorShiftEvidence) -> String {
    let current_pct = evidence
        .current_error_ratio
        .map(|value| format!("{:.1}%", value * 100.0))
        .unwrap_or_else(|| "Unavailable".into());
    let baseline_pct = evidence
        .baseline_average_error_ratio
        .map(|value| format!("{:.1}%", value * 100.0))
        .unwrap_or_else(|| "Unavailable".into());
    format!(
        "<div class=\"evidence-summary\">\
           <strong>{errors} of {total} recent messages were errors ({current}).</strong>\
           <span>Baseline: {baseline} average per window; {baseline_errors} errors in {baseline_messages} messages across {samples} prior observation windows.</span>\
         </div>",
        errors = evidence.current_errors,
        total = evidence.current_total,
        current = escape_html(&current_pct),
        baseline = escape_html(&baseline_pct),
        baseline_errors = evidence.baseline_errors,
        baseline_messages = evidence.baseline_messages,
        samples = evidence.baseline_window_samples,
    )
}

fn render_card(finding: &DashboardFinding, evidence: Option<&DashboardEvidence>) -> String {
    let href = finding_href(&finding.finding_key);
    let evidence_summary = match evidence {
        Some(DashboardEvidence::ErrorShift(value)) => render_error_shift_summary(value),
        None => format!(
            "<p class=\"observed-claim\">{}</p>",
            escape_html(&finding.message)
        ),
    };
    let unknown_items = unknowns(finding)
        .into_iter()
        .map(|item| format!("<li>{}</li>", escape_html(&item)))
        .collect::<String>();
    let meta = nq_db::finding_meta::finding_meta(&finding.kind);
    let next_checks = meta
        .next_checks
        .iter()
        .map(|check| format!("<li>{}</li>", escape_html(check)))
        .collect::<String>();
    let observation_time = finding
        .current_observation
        .as_ref()
        .map(|observation| observation.observed_at.as_str())
        .unwrap_or(&finding.last_seen_at);
    let age = finding
        .observation_age_seconds
        .map(human_duration)
        .unwrap_or_else(|| "unknown age".into());
    let response = response_label(finding.diagnosis.action_bias.as_deref());
    let response = if finding.status == DashboardFindingStatus::Ongoing {
        response.to_string()
    } else {
        format!("Recorded response when observed: {response}")
    };

    format!(
        "<article class=\"finding-card {state_class}\" data-finding-key=\"{key}\" data-finding-state=\"{state}\">\
           <div class=\"card-status\"><span>{state}</span><span>{response}</span></div>\
           <h3><a href=\"{href}\">{title}</a></h3>\
           {evidence_summary}\
           <dl class=\"decision-facts\">\
             <div><dt>Affected</dt><dd>{host}{subject}</dd></div>\
             <div><dt>Observed</dt><dd><time datetime=\"{observed_at}\">{observed_at}</time> ({age} ago)</dd></div>\
             <div><dt>Impact</dt><dd>{impact}</dd></div>\
           </dl>\
           <div class=\"unknowns\"><strong>What remains unknown</strong><ul>{unknown_items}</ul></div>\
           <details class=\"next-inspection\"><summary>Recommended next inspection</summary><ul>{next_checks}</ul></details>\
           <div class=\"card-actions\"><a class=\"primary-link\" href=\"{href}\">Investigate evidence</a></div>\
           <details class=\"advanced\"><summary>Advanced NQ classification</summary>\
             <dl>\
               <div><dt>Operator translation</dt><dd>{domain_plain}</dd></div>\
               <div><dt>Delta class</dt><dd><code>{domain}</code></dd></div>\
               <div><dt>Detector</dt><dd><code>{kind}</code></dd></div>\
               <div><dt>Stable finding identity</dt><dd><code>{key}</code></dd></div>\
               <div><dt>Last observation snapshot</dt><dd>#{last_generation}</dd></div>\
             </dl>\
           </details>\
         </article>",
        state_class = status_class(finding.status),
        key = escape_html(&finding.finding_key),
        state = escape_html(status_label(finding.status)),
        response = escape_html(&response),
        href = escape_html(&href),
        title = finding_title(finding),
        evidence_summary = evidence_summary,
        host = if finding.host.is_empty() {
            "NQ".into()
        } else {
            escape_html(&finding.host)
        },
        subject = if finding.subject.is_empty() {
            String::new()
        } else {
            format!(" / {}", escape_html(&finding.subject))
        },
        observed_at = escape_html(observation_time),
        age = escape_html(&age),
        impact = escape_html(&impact_statement(finding)),
        unknown_items = unknown_items,
        next_checks = next_checks,
        domain_plain = escape_html(plain_domain(&finding.domain)),
        domain = escape_html(&finding.domain),
        kind = escape_html(&finding.kind),
        last_generation = finding.last_seen_generation,
    )
}

fn render_inventory(inventory: &DashboardInventory) -> String {
    let hosts = inventory
        .hosts
        .iter()
        .map(|host| {
            let stale_label = if host.display_stale {
                "<strong class=\"unknown-value\">Stale inventory.</strong> "
            } else {
                ""
            };
            format!(
                "<tr class=\"{}\"><th scope=\"row\">{}</th><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}<time datetime=\"{}\">{}</time> ({})</td></tr>",
                if host.display_stale { "inventory-stale" } else { "" },
                escape_html(&host.host),
                optional_number(host.cpu_load_1m.map(|v| format!("{v:.1}")), ""),
                optional_number(host.mem_pressure_pct.map(|v| format!("{v:.1}")), "%"),
                optional_number(host.disk_used_pct.map(|v| format!("{v:.1}")), "%"),
                optional_number(host.disk_available_mb, " MB"),
                stale_label,
                escape_html(&host.collected_at),
                escape_html(&host.collected_at),
                escape_html(&host.age_seconds.map(human_duration).unwrap_or_else(|| "age unavailable".into())),
            )
        })
        .collect::<String>();
    let services = inventory
        .services
        .iter()
        .map(|service| {
            let stale_label = if service.display_stale {
                "<strong class=\"unknown-value\">Stale inventory.</strong> "
            } else {
                ""
            };
            format!(
                "<tr class=\"{}\"><th scope=\"row\">{}</th><td>{}</td><td>{}</td><td>{}</td><td>{}<time datetime=\"{}\">{}</time> ({})</td></tr>",
                if service.display_stale { "inventory-stale" } else { "" },
                escape_html(&service.host),
                escape_html(&service.service),
                escape_html(&service.service_status),
                optional_number(service.queue_depth, ""),
                stale_label,
                escape_html(&service.collected_at),
                escape_html(&service.collected_at),
                escape_html(&service.age_seconds.map(human_duration).unwrap_or_else(|| "age unavailable".into())),
            )
        })
        .collect::<String>();
    let databases = inventory
        .sqlite_databases
        .iter()
        .map(|database| {
            let stale_label = if database.display_stale {
                "<strong class=\"unknown-value\">Stale inventory.</strong> "
            } else {
                ""
            };
            format!(
                "<tr class=\"{}\"><th scope=\"row\">{}</th><td class=\"long-value\">{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}<time datetime=\"{}\">{}</time> ({})</td></tr>",
                if database.display_stale { "inventory-stale" } else { "" },
                escape_html(&database.host),
                escape_html(&database.db_path),
                optional_number(database.db_size_mb.map(|v| format!("{v:.1}")), " MB"),
                optional_number(database.wal_size_mb.map(|v| format!("{v:.1}")), " MB"),
                optional_text(database.last_quick_check.as_deref()),
                stale_label,
                escape_html(&database.collected_at),
                escape_html(&database.collected_at),
                escape_html(&database.age_seconds.map(human_duration).unwrap_or_else(|| "age unavailable".into())),
            )
        })
        .collect::<String>();

    format!(
        "<details class=\"inventory\" id=\"inventory\">\
           <summary>Inventory and exploration</summary>\
           <p>Inventory is supporting context. Each row keeps its own observation time; an empty value means unavailable, not zero.</p>\
           <div class=\"table-scroll\"><table><caption>Host inventory</caption><thead><tr><th scope=\"col\">Host</th><th scope=\"col\">CPU 1m</th><th scope=\"col\">Memory</th><th scope=\"col\">Disk used</th><th scope=\"col\">Disk free</th><th scope=\"col\">Observed</th></tr></thead><tbody>{hosts}</tbody></table></div>\
           <div class=\"table-scroll\"><table><caption>Service inventory</caption><thead><tr><th scope=\"col\">Host</th><th scope=\"col\">Service</th><th scope=\"col\">Status</th><th scope=\"col\">Queue</th><th scope=\"col\">Observed</th></tr></thead><tbody>{services}</tbody></table></div>\
           <div class=\"table-scroll\"><table><caption>SQLite inventory</caption><thead><tr><th scope=\"col\">Host</th><th scope=\"col\">Database</th><th scope=\"col\">Size</th><th scope=\"col\">WAL</th><th scope=\"col\">Quick check</th><th scope=\"col\">Observed</th></tr></thead><tbody>{databases}</tbody></table></div>\
           <details class=\"expert-tools\"><summary>Expert SQL and raw inspection</summary>\
             <p>SQL is an expert tool, not a prerequisite for understanding a finding. Queries are read-only.</p>\
             <form class=\"sql-form\" onsubmit=\"runExpertQuery(event)\">\
               <label for=\"expert-sql\">Read-only SQL</label>\
               <textarea id=\"expert-sql\" name=\"sql\" rows=\"3\">SELECT * FROM v_warnings</textarea>\
               <button type=\"submit\">Run read-only query</button>\
             </form><pre id=\"expert-result\" aria-live=\"polite\"></pre>\
           </details>\
         </details>",
        hosts = hosts,
        services = services,
        databases = databases,
    )
}

fn page_styles() -> &'static str {
    r#"
:root {
  color-scheme: dark;
  --bg: #0b1016;
  --panel: #121923;
  --panel-strong: #172131;
  --text: #eef3f8;
  --muted: #aebac8;
  --faint: #8795a5;
  --border: #354457;
  --accent: #76b7ff;
  --attention: #ffd166;
  --danger: #ff8b8b;
  --ok: #8bd9ad;
}
* { box-sizing: border-box; }
html { background: var(--bg); }
body {
  margin: 0;
  background: var(--bg);
  color: var(--text);
  font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  font-size: 16px;
  line-height: 1.5;
}
a { color: var(--accent); }
a:hover { text-decoration-thickness: 2px; }
a:focus-visible, button:focus-visible, summary:focus-visible, textarea:focus-visible {
  outline: 3px solid #ffffff;
  outline-offset: 3px;
}
code, pre, textarea, .expert-token {
  font-family: ui-monospace, SFMono-Regular, Consolas, "Liberation Mono", monospace;
}
.site-header {
  border-bottom: 1px solid var(--border);
  background: #0d141e;
}
.site-header-inner, main, .site-footer {
  width: min(1120px, calc(100% - 32px));
  margin-inline: auto;
}
.site-header-inner {
  padding: 18px 0;
  display: flex;
  gap: 24px;
  justify-content: space-between;
  align-items: center;
}
.brand { color: var(--text); font-weight: 800; text-decoration: none; font-size: 20px; }
.snapshot-basis {
  display: flex;
  flex-wrap: wrap;
  gap: 7px 12px;
  align-items: baseline;
  color: var(--muted);
  font-size: 14px;
}
.basis-label { color: var(--text); font-weight: 700; }
.expert-token { color: var(--faint); font-size: 12px; }
main { padding: 32px 0 56px; }
.eyebrow { color: var(--muted); font-size: 13px; font-weight: 700; letter-spacing: .08em; text-transform: uppercase; }
h1, h2, h3 { line-height: 1.2; }
h1 { margin: 6px 0 10px; font-size: clamp(28px, 5vw, 42px); }
h2 { margin-top: 40px; font-size: 24px; }
h3 { margin: 12px 0 10px; font-size: clamp(19px, 3vw, 24px); }
.section-intro { color: var(--muted); max-width: 72ch; }
.finding-grid { display: grid; gap: 18px; }
.finding-card {
  background: var(--panel);
  border: 1px solid var(--border);
  border-left: 6px solid var(--attention);
  border-radius: 10px;
  padding: 20px;
}
.finding-card.state-stale, .finding-card.state-unknown { border-left-style: dashed; }
.finding-card.state-historical { border-left-color: var(--faint); }
.finding-card.state-recovering { border-left-color: var(--ok); }
.card-status {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 16px;
  color: var(--muted);
  font-size: 14px;
  font-weight: 700;
}
.evidence-summary, .observed-claim {
  display: grid;
  gap: 3px;
  margin: 12px 0;
  color: var(--text);
}
.evidence-summary span { color: var(--muted); }
.decision-facts, .advanced dl {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
  margin: 16px 0;
}
.decision-facts div, .advanced dl div {
  min-width: 0;
  border-top: 1px solid var(--border);
  padding-top: 8px;
}
dt { color: var(--muted); font-size: 13px; font-weight: 700; }
dd { margin: 2px 0 0; overflow-wrap: anywhere; }
.unknowns {
  background: #1a2029;
  border: 1px dashed #68788b;
  border-radius: 8px;
  padding: 12px 14px;
}
.unknowns ul, .next-inspection ul { margin: 7px 0 0 20px; }
.next-inspection, .advanced { margin-top: 14px; }
summary { cursor: pointer; font-weight: 700; }
.advanced { color: var(--muted); }
.card-actions { margin-top: 16px; }
.primary-link, button {
  display: inline-block;
  border: 1px solid #5595da;
  border-radius: 7px;
  background: #17345a;
  color: #f7fbff;
  padding: 9px 14px;
  font: inherit;
  font-weight: 700;
  text-decoration: none;
  cursor: pointer;
}
button[disabled] { cursor: not-allowed; opacity: .6; }
.empty-state, .missing-state, .history-state {
  border: 1px dashed var(--border);
  border-radius: 10px;
  background: var(--panel);
  padding: 22px;
}
.self-health {
  border: 1px solid #6b5d91;
  background: #171524;
  border-radius: 10px;
  padding: 18px;
}
.self-health .finding-card { background: #14131e; }
.inventory {
  margin-top: 42px;
  border-top: 1px solid var(--border);
  padding-top: 20px;
}
.inventory > summary { font-size: 21px; }
.table-scroll { overflow-x: auto; margin: 18px 0; }
table { width: 100%; border-collapse: collapse; font-size: 14px; }
caption { text-align: left; font-weight: 800; font-size: 17px; margin-bottom: 7px; }
th, td { border-bottom: 1px solid var(--border); padding: 8px 10px; text-align: left; vertical-align: top; }
.inventory-stale th, .inventory-stale td { text-decoration: underline dotted; text-decoration-color: var(--attention); }
.unknown-value { color: var(--attention); font-style: italic; }
.long-value, code { overflow-wrap: anywhere; }
.expert-tools { margin-top: 20px; }
.sql-form { display: grid; gap: 9px; }
textarea { width: 100%; background: #080d13; color: var(--text); border: 1px solid var(--border); border-radius: 7px; padding: 10px; }
pre { white-space: pre-wrap; max-height: 420px; overflow: auto; color: var(--muted); }
.site-footer { padding: 22px 0 36px; color: var(--muted); border-top: 1px solid var(--border); font-size: 13px; }
.back-link { display: inline-block; margin-bottom: 18px; }
.state-banner { border: 2px solid var(--attention); border-radius: 9px; padding: 14px; margin: 16px 0; }
.state-banner strong { display: block; font-size: 18px; }
.evidence-panel, .action-panel, .advanced-panel { margin-top: 28px; }
.evidence-table td:first-child, .evidence-table th:first-child { width: 34%; }
.history-list { padding-left: 20px; }
.history-list time { color: var(--muted); }
.action-unavailable { border: 1px dashed var(--border); padding: 14px; border-radius: 8px; color: var(--muted); }
dialog { width: min(640px, calc(100% - 24px)); border: 2px solid var(--border); border-radius: 10px; background: var(--panel-strong); color: var(--text); padding: 22px; }
dialog::backdrop { background: rgb(0 0 0 / .75); }
.dialog-actions { display: flex; flex-wrap: wrap; gap: 10px; justify-content: flex-end; margin-top: 18px; }
.secondary-button { background: transparent; border-color: var(--border); }
.action-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }
.action-choice { border: 1px solid var(--border); border-radius: 8px; padding: 13px; }
.action-choice p { color: var(--muted); margin: 5px 0 10px; }
.action-choice h3 { margin-top: 0; font-size: 18px; }
.effect-columns { display: grid; grid-template-columns: 1fr 1fr; gap: 18px; }
.effect-columns section { border: 1px solid var(--border); border-radius: 8px; padding: 12px; }
.effect-columns h3 { margin-top: 0; font-size: 17px; }
.action-fields { display: grid; gap: 7px; margin-top: 16px; }
.action-fields input {
  width: 100%;
  border: 1px solid var(--border);
  border-radius: 7px;
  background: #080d13;
  color: var(--text);
  font: inherit;
  padding: 9px 10px;
}
.permission-note, .action-precondition, .evidence-caution {
  border: 1px dashed #68788b;
  border-radius: 8px;
  padding: 11px 13px;
}
.action-status { color: var(--attention); font-weight: 700; }
.conflict-state {
  border: 2px solid var(--attention);
  border-radius: 8px;
  padding: 14px;
}
@media (max-width: 720px) {
  .site-header-inner { align-items: flex-start; flex-direction: column; }
  .decision-facts, .advanced dl, .action-grid, .effect-columns { grid-template-columns: 1fr; }
  main { padding-top: 22px; }
  .finding-card { padding: 16px; }
}
"#
}

fn page_script() -> &'static str {
    r#"
async function runExpertQuery(event) {
  event.preventDefault();
  const field = document.getElementById('expert-sql');
  const result = document.getElementById('expert-result');
  result.textContent = 'Running…';
  try {
    const response = await fetch('/api/query?sql=' + encodeURIComponent(field.value));
    const data = await response.json();
    if (!response.ok || data.error) {
      result.textContent = 'Query unavailable: ' + (data.error || response.status);
      return;
    }
    const rows = data.rows || [];
    result.textContent = [data.columns.join(' | ')].concat(rows.map(row => row.join(' | '))).join('\n') || '(no rows)';
  } catch (error) {
    result.textContent = 'Query unavailable.';
  }
}

function optionalField(dialog, name) {
  const field = dialog.querySelector('[name="' + name + '"]');
  if (!field) return null;
  const value = field.value.trim();
  return value === '' ? null : value;
}

function actionRequest(dialog) {
  const ttlField = dialog.querySelector('[name="ttl_hours"]');
  const ttlValue = ttlField && ttlField.value !== '' ? Number(ttlField.value) : null;
  return {
    finding_key: dialog.dataset.findingKey,
    action: dialog.dataset.action,
    expected_work_state: dialog.dataset.expectedWorkState,
    expected_last_seen_gen: Number(dialog.dataset.expectedLastSeenGen),
    note: optionalField(dialog, 'note'),
    owner: optionalField(dialog, 'owner'),
    actor: optionalField(dialog, 'actor'),
    ttl_hours: ttlValue
  };
}

function invalidateActionPreview(field) {
  const dialog = field.closest('dialog');
  dialog.querySelector('.confirm-action').disabled = true;
  dialog.querySelector('.action-status').textContent =
    'Inputs changed. Validate the preview again before confirming.';
}

async function previewFindingAction(button) {
  const dialog = document.getElementById(button.dataset.dialog);
  const status = dialog.querySelector('.action-status');
  const confirm = dialog.querySelector('.confirm-action');
  confirm.disabled = true;
  status.textContent = 'Checking whether this target is still actionable…';
  if (!dialog.open) dialog.showModal();
  try {
    const response = await fetch('/api/finding/action/preview', {
      method: 'POST',
      headers: {'content-type': 'application/json'},
      body: JSON.stringify(actionRequest(dialog))
    });
    const data = await response.json();
    if (!response.ok || !data.ok) {
      status.textContent = 'Action unavailable: ' + (data.error || response.status);
      return;
    }
    const expiry = data.preview.expires_at
      ? ' Expiry will be recorded for ' + data.preview.expires_at + '.'
      : ' No automatic expiry was requested.';
    status.textContent =
      'Target and preconditions validated.' + expiry +
      ' Review the effects above, then confirm or cancel.';
    confirm.disabled = false;
    confirm.focus();
  } catch (error) {
    status.textContent = 'Action preview unavailable. Nothing was changed.';
  }
}

async function applyFindingAction(button) {
  const dialog = button.closest('dialog');
  const status = dialog.querySelector('.action-status');
  button.disabled = true;
  status.textContent = 'Re-checking target and recording the transition…';
  try {
    const response = await fetch('/api/finding/action', {
      method: 'POST',
      headers: {'content-type': 'application/json'},
      body: JSON.stringify(actionRequest(dialog))
    });
    const data = await response.json();
    if (!response.ok || !data.ok) {
      status.textContent = 'No change applied: ' + (data.error || response.status);
      return;
    }
    status.textContent = 'Recorded: ' + data.receipt.from_work_state + ' → ' + data.receipt.to_work_state + '. Reloading durable state and history…';
    window.setTimeout(function () { window.location.reload(); }, 900);
  } catch (error) {
    status.textContent = 'The action result could not be confirmed. Reload before retrying; do not assume it changed.';
  }
}
"#
}

fn page_shell(title: &str, basis: &DashboardBasis, content: String) -> String {
    format!(
        "<!doctype html>\
         <html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{title} — NQ</title><style>{styles}</style></head>\
         <body><header class=\"site-header\"><div class=\"site-header-inner\">\
           <a class=\"brand\" href=\"/\" aria-label=\"NQ dashboard home\">NQ</a>{basis}\
         </div></header>{content}\
         <footer class=\"site-footer\">NQ reports bounded observations. A finding does not by itself prove cause, user impact, or authorization to change a monitored system.</footer>\
         <script>{script}</script></body></html>",
        title = escape_html(title),
        styles = page_styles(),
        basis = render_basis(basis),
        content = content,
        script = page_script(),
    )
}

pub fn render_overview(overview: &DashboardOverview) -> String {
    let attention = overview
        .monitored_findings
        .iter()
        .filter(|finding| needs_attention(finding))
        .collect::<Vec<_>>();
    let unknown = overview
        .monitored_findings
        .iter()
        .filter(|finding| is_unknown_state(finding))
        .collect::<Vec<_>>();
    let changed = overview
        .monitored_findings
        .iter()
        .filter(|finding| {
            !needs_attention(finding) && !is_unknown_state(finding) && is_recent_change(finding)
        })
        .collect::<Vec<_>>();
    let watching = overview
        .monitored_findings
        .iter()
        .filter(|finding| {
            !needs_attention(finding) && !is_unknown_state(finding) && !is_recent_change(finding)
        })
        .collect::<Vec<_>>();

    let attention_cards = attention
        .iter()
        .map(|finding| render_card(finding, finding.evidence.as_ref()))
        .collect::<String>();
    let unknown_cards = unknown
        .iter()
        .map(|finding| render_card(finding, finding.evidence.as_ref()))
        .collect::<String>();
    let changed_cards = changed
        .iter()
        .map(|finding| render_card(finding, finding.evidence.as_ref()))
        .collect::<String>();
    let watching_cards = watching
        .iter()
        .map(|finding| render_card(finding, finding.evidence.as_ref()))
        .collect::<String>();
    let self_health_cards = overview
        .nq_self_health
        .iter()
        .map(|finding| render_card(finding, finding.evidence.as_ref()))
        .collect::<String>();

    let attention_summary = match attention.len() {
        0 => "No current issue is supported by this snapshot.".to_string(),
        1 => "1 issue needs attention.".to_string(),
        count => format!("{count} issues need attention."),
    };
    let attention_body = if attention.is_empty() {
        "<div class=\"empty-state\"><strong>No monitored-system issue currently requires action.</strong><p>This is bounded by the observation basis and coverage shown below; it is not a universal health claim.</p></div>".into()
    } else {
        format!("<div class=\"finding-grid\">{attention_cards}</div>")
    };
    let unknown_section = if unknown.is_empty() {
        String::new()
    } else {
        format!(
            "<section aria-labelledby=\"unknown-heading\"><h2 id=\"unknown-heading\">Unknowns blocking decisions ({})</h2>\
             <p class=\"section-intro\">These conditions cannot honestly be presented as current health or current failure.</p>\
             <div class=\"finding-grid\">{unknown_cards}</div></section>",
            unknown.len()
        )
    };
    let changed_section = if changed.is_empty() {
        String::new()
    } else {
        format!(
            "<section aria-labelledby=\"changed-heading\"><h2 id=\"changed-heading\">Recently changed ({})</h2>\
             <div class=\"finding-grid\">{changed_cards}</div></section>",
            changed.len()
        )
    };
    let watching_section = if watching.is_empty() {
        String::new()
    } else {
        format!(
            "<section aria-labelledby=\"watching-heading\"><h2 id=\"watching-heading\">Watching ({})</h2>\
             <p class=\"section-intro\">Current conditions that do not presently call for the same response as the attention queue.</p>\
             <div class=\"finding-grid\">{watching_cards}</div></section>",
            watching.len()
        )
    };
    let self_health_section = if overview.nq_self_health.is_empty() {
        "<section class=\"self-health\" aria-labelledby=\"self-health-heading\"><h2 id=\"self-health-heading\">NQ system health</h2><p>No NQ self-health finding is recorded in this snapshot.</p></section>".into()
    } else {
        format!(
            "<section class=\"self-health\" aria-labelledby=\"self-health-heading\"><h2 id=\"self-health-heading\">NQ system health ({})</h2>\
             <p>Problems in collection, evaluation, or NQ's own observation path. These are not monitored-service incidents.</p>\
             <div class=\"finding-grid\">{self_health_cards}</div></section>",
            overview.nq_self_health.len()
        )
    };

    let content = format!(
        "<main id=\"main-content\">\
           <section aria-labelledby=\"attention-heading\">\
             <div class=\"eyebrow\">Decision</div><h1 id=\"attention-heading\">{attention_summary}</h1>\
             <p class=\"section-intro\">Start with the operational claim. Evidence, uncertainty, and expert classification remain attached to it.</p>\
             {attention_body}\
           </section>\
           {unknown_section}{changed_section}{watching_section}{self_health_section}\
           {inventory}\
         </main>",
        attention_summary = escape_html(&attention_summary),
        attention_body = attention_body,
        unknown_section = unknown_section,
        changed_section = changed_section,
        watching_section = watching_section,
        self_health_section = self_health_section,
        inventory = render_inventory(&overview.inventory),
    );
    page_shell("Dashboard", &overview.basis, content)
}

fn percentage(value: Option<f64>) -> String {
    value
        .map(|value| format!("{:.1}%", value * 100.0))
        .unwrap_or_else(|| "<span class=\"unknown-value\">Unavailable</span>".into())
}

fn action_name(action: FindingAction) -> &'static str {
    match action {
        FindingAction::Acknowledge => "acknowledge",
        FindingAction::Watch => "watch",
        FindingAction::Quiesce => "quiesce",
        FindingAction::Close => "close",
        FindingAction::Suppress => "suppress",
        FindingAction::Reset => "reset",
    }
}

fn render_effect_list(items: &[&str]) -> String {
    items
        .iter()
        .map(|item| format!("<li>{}</li>", escape_html(item)))
        .collect()
}

fn render_action_choice(finding: &DashboardFinding, contract: &FindingActionContract) -> String {
    let action = action_name(contract.action);
    let dialog_id = format!("action-dialog-{action}");
    let will = render_effect_list(contract.will);
    let will_not = render_effect_list(contract.will_not);
    let ttl = match contract.ttl_policy {
        TtlPolicy::Unsupported => String::new(),
        TtlPolicy::OptionalBounded {
            min_hours,
            max_hours,
        } => format!(
            "<label for=\"ttl-{action}\">Optional expiry in hours ({min_hours}–{max_hours})</label>\
             <input id=\"ttl-{action}\" name=\"ttl_hours\" type=\"number\" min=\"{min_hours}\" max=\"{max_hours}\" inputmode=\"numeric\" oninput=\"invalidateActionPreview(this)\">"
        ),
    };

    format!(
        "<div class=\"action-choice\">\
           <h3>{label}</h3><p>{summary}</p>\
           <button type=\"button\" aria-haspopup=\"dialog\" aria-controls=\"{dialog_id}\"\
             data-dialog=\"{dialog_id}\" onclick=\"previewFindingAction(this)\">Preview {label}</button>\
         </div>\
         <dialog id=\"{dialog_id}\"\
           data-finding-key=\"{key}\"\
           data-action=\"{action}\"\
           data-expected-work-state=\"{work_state}\"\
           data-expected-last-seen-gen=\"{generation}\">\
           <h2>{label} this finding?</h2>\
           <p>{summary}</p>\
           <dl class=\"decision-facts\">\
             <div><dt>Concrete target</dt><dd><code>{key}</code></dd></div>\
             <div><dt>State transition</dt><dd><code>{work_state}</code> → <code>{target_state}</code></dd></div>\
             <div><dt>Reversible</dt><dd>{reversible}</dd></div>\
           </dl>\
           <div class=\"effect-columns\">\
             <section><h3>This will</h3><ul>{will}</ul></section>\
             <section><h3>This will not</h3><ul>{will_not}</ul></section>\
           </div>\
           <p class=\"permission-note\"><strong>Access:</strong> the server has local dashboard write access. NQ does not authenticate an individual operator on this route; the supplied actor is an audit label, not proof of identity.</p>\
           <div class=\"action-fields\">\
             <label for=\"actor-{action}\">Actor recorded in history</label>\
             <input id=\"actor-{action}\" name=\"actor\" value=\"dashboard-local-operator\" required oninput=\"invalidateActionPreview(this)\">\
             <label for=\"owner-{action}\">Owner (optional)</label>\
             <input id=\"owner-{action}\" name=\"owner\" oninput=\"invalidateActionPreview(this)\">\
             <label for=\"note-{action}\">Audit note (optional)</label>\
             <textarea id=\"note-{action}\" name=\"note\" rows=\"2\" oninput=\"invalidateActionPreview(this)\"></textarea>\
             {ttl}\
           </div>\
           <p class=\"action-precondition\">NQ will re-check this exact finding, work state, latest observation snapshot, visibility, presence, and evidence basis before changing anything.</p>\
           <p class=\"action-status\" role=\"status\" aria-live=\"polite\">Checking whether this target is still actionable…</p>\
           <div class=\"dialog-actions\">\
             <button type=\"button\" class=\"secondary-button\" onclick=\"this.closest('dialog').close()\">Cancel</button>\
             <button type=\"button\" class=\"secondary-button\" data-dialog=\"{dialog_id}\" onclick=\"previewFindingAction(this)\">Validate preview</button>\
             <button type=\"button\" class=\"confirm-action\" disabled onclick=\"applyFindingAction(this)\">Confirm {label}</button>\
           </div>\
         </dialog>",
        label = escape_html(contract.label),
        summary = escape_html(contract.summary),
        dialog_id = dialog_id,
        key = escape_html(&finding.finding_key),
        action = action,
        work_state = escape_html(&finding.work_state),
        generation = finding.last_seen_generation,
        target_state = escape_html(contract.target_work_state.as_str()),
        reversible = if contract.reversible { "Yes" } else { "No" },
        will = will,
        will_not = will_not,
        ttl = ttl,
    )
}

fn render_actions(
    finding: &DashboardFinding,
    basis: &DashboardBasis,
    mutation_available: bool,
) -> String {
    let basis_current = basis.status.as_deref() == Some("complete")
        && basis.generation_id == Some(finding.last_seen_generation)
        && basis
            .age_seconds
            .is_some_and(|age| age <= nq_db::dashboard::DASHBOARD_STALE_AFTER_SECONDS);
    let finding_current = finding.status == DashboardFindingStatus::Ongoing
        && !finding.display_stale
        && finding.visibility_state == "observed"
        && finding.absent_generations == 0
        && finding.basis_state == "live";

    if !mutation_available {
        return "<div class=\"action-unavailable\"><strong>Actions unavailable.</strong> This dashboard was opened without write access. Evidence and history remain inspectable.</div>".into();
    }
    if !basis_current || !finding_current {
        return "<div class=\"action-unavailable\"><strong>Actions disabled for safety.</strong> The finding is stale, recovering, suppressed, historical, or not bound to the latest complete observation snapshot. Reload or investigate the evidence; no coordination state will be changed.</div>".into();
    }
    if !matches!(
        finding.work_state.as_str(),
        "new" | "acknowledged" | "watching" | "quiesced" | "closed" | "suppressed"
    ) {
        return "<div class=\"action-unavailable\"><strong>Actions disabled for safety.</strong> The stored coordination state is not recognized.</div>".into();
    }

    let choices = FindingAction::ALL
        .iter()
        .map(|action| render_action_choice(finding, &action.contract()))
        .collect::<String>();
    format!(
        "<p>These controls change only operator coordination and notification eligibility for the concrete finding shown above. They do not act on the monitored system.</p>\
         <div class=\"action-grid\">{choices}</div>"
    )
}

fn render_error_shift_evidence(evidence: &nq_db::dashboard::ErrorShiftEvidence) -> String {
    let comparison_range = match (
        evidence.comparison_basis.generation_start,
        evidence.comparison_basis.generation_end,
    ) {
        (Some(start), Some(end)) => format!("snapshots #{start}–#{end}"),
        _ => "retained comparison range unavailable".into(),
    };
    let sufficiency = if evidence.baseline_average_error_ratio.is_none() {
        format!(
            "Insufficient comparison coverage: {} prior windows are retained; at least 3 are required.",
            evidence.baseline_window_samples
        )
    } else if evidence.current_sample_size < 20 {
        format!(
            "Low current sample: only {} messages were observed. Treat the magnitude cautiously.",
            evidence.current_sample_size
        )
    } else {
        "The detector's minimum comparison coverage is present; this does not establish cause or impact."
            .into()
    };
    let examples = if evidence.examples.is_empty() {
        "<p class=\"unknown-value\">No parseable error examples are attached to this observation.</p>"
            .into()
    } else {
        let rows = evidence
            .examples
            .iter()
            .map(|example| {
                format!(
                    "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                    optional_text(example.timestamp.as_deref()),
                    optional_text(example.severity.as_deref()),
                    optional_text(example.message.as_deref()),
                )
            })
            .collect::<String>();
        format!(
            "<div class=\"table-scroll\"><table><caption>Error examples from the current source window</caption>\
             <thead><tr><th scope=\"col\">Time</th><th scope=\"col\">Level</th><th scope=\"col\">Message</th></tr></thead>\
             <tbody>{rows}</tbody></table></div>"
        )
    };

    format!(
        "<div class=\"table-scroll\"><table class=\"evidence-table\">\
           <caption>Error-rate comparison</caption><tbody>\
             <tr><th scope=\"row\">Current observation</th><td>{errors} errors in {total} messages ({current})</td></tr>\
             <tr><th scope=\"row\">Established comparison</th><td>{baseline} average per window; {baseline_errors} errors in {baseline_messages} messages across {baseline_samples} prior windows, {comparison_range}</td></tr>\
             <tr><th scope=\"row\">Current source window</th><td><time datetime=\"{window_start}\">{window_start}</time> to <time datetime=\"{window_end}\">{window_end}</time></td></tr>\
             <tr><th scope=\"row\">Source</th><td><code>{source}</code>, collected <time datetime=\"{source_at}\">{source_at}</time>, snapshot #{generation}</td></tr>\
             <tr><th scope=\"row\">Comparison rule</th><td>{description}</td></tr>\
           </tbody></table></div>\
         <div class=\"evidence-caution\"><strong>Evidence sufficiency</strong><p>{sufficiency}</p></div>\
         {examples}\
         <p><strong>Bounded conclusion:</strong> the observed error proportion changed relative to the detector comparison. This evidence does not identify the cause, prove a deployment was responsible, or establish user-visible impact.</p>",
        errors = evidence.current_errors,
        total = evidence.current_total,
        current = percentage(evidence.current_error_ratio),
        baseline = percentage(evidence.baseline_average_error_ratio),
        baseline_errors = evidence.baseline_errors,
        baseline_messages = evidence.baseline_messages,
        baseline_samples = evidence.baseline_window_samples,
        comparison_range = escape_html(&comparison_range),
        window_start = escape_html(&evidence.window_start),
        window_end = escape_html(&evidence.window_end),
        source = escape_html(&evidence.source_id),
        source_at = escape_html(&evidence.source_observed_at),
        generation = evidence.current_generation,
        description = escape_html(&evidence.comparison_basis.description),
        sufficiency = escape_html(&sufficiency),
        examples = examples,
    )
}

fn render_evidence(finding: &DashboardFinding, evidence: Option<&DashboardEvidence>) -> String {
    match evidence {
        Some(DashboardEvidence::ErrorShift(evidence)) => render_error_shift_evidence(evidence),
        None if finding.kind == "smart_status_lies" => format!(
            "<div class=\"conflict-state\"><strong>Sources disagree.</strong>\
             <p>{}</p><p>NQ preserves the contradiction; it does not average these channels into a healthy or failed verdict.</p></div>\
             <p><strong>Detector output:</strong> {}</p>",
            escape_html(nq_db::finding_meta::finding_meta(&finding.kind).contradiction),
            escape_html(&finding.message),
        ),
        None => format!(
            "<p><strong>Detector output:</strong> {}</p>\
             <div class=\"evidence-caution\"><strong>Structured evidence unavailable on this dashboard view.</strong>\
             <p>The finding remains inspectable through its retained observations below. An unavailable value is not rendered as zero or healthy.</p></div>",
            escape_html(&finding.message)
        ),
    }
}

fn render_observation_history(history: &nq_db::dashboard::DashboardObservationHistory) -> String {
    if history.entries.is_empty() {
        return "<p class=\"unknown-value\">No retained finding observations are available.</p>"
            .into();
    }
    let entries = history
        .entries
        .iter()
        .map(|entry| {
            format!(
                "<li><time datetime=\"{at}\">{at}</time> · snapshot #{generation} · value {value}<div>{message}</div></li>",
                at = escape_html(&entry.observed_at),
                generation = entry.generation_id,
                value = optional_number(entry.value, ""),
                message = optional_text(entry.message.as_deref()),
            )
        })
        .collect::<String>();
    format!(
        "<ol class=\"history-list\">{entries}</ol><p>{shown} of {total} retained observations shown{truncated}.</p>",
        shown = history.entries.len(),
        total = history.total_count,
        truncated = if history.truncated {
            "; older entries are not rendered"
        } else {
            ""
        },
    )
}

fn render_transition_history(history: &nq_db::dashboard::DashboardTransitionHistory) -> String {
    if history.entries.is_empty() {
        return "<p>No operator coordination transitions have been recorded.</p>".into();
    }
    let entries = history
        .entries
        .iter()
        .map(|entry| {
            format!(
                "<li><time datetime=\"{at}\">{at}</time> · <code>{from}</code> → <code>{to}</code> · actor {actor}<div>{note}</div></li>",
                at = escape_html(&entry.created_at),
                from = optional_text(entry.from_state.as_deref()),
                to = escape_html(&entry.to_state),
                actor = optional_text(entry.changed_by.as_deref()),
                note = optional_text(entry.note.as_deref()),
            )
        })
        .collect::<String>();
    format!(
        "<ol class=\"history-list\">{entries}</ol><p>{shown} of {total} durable transitions shown{truncated}.</p>",
        shown = history.entries.len(),
        total = history.total_count,
        truncated = if history.truncated {
            "; older entries are not rendered"
        } else {
            ""
        },
    )
}

fn render_current_detail(
    detail: &DashboardCurrentFindingDetail,
    mutation_available: bool,
) -> String {
    let finding = &detail.finding;
    let meta = nq_db::finding_meta::finding_meta(&finding.kind);
    let unknown_items = unknowns(finding)
        .into_iter()
        .map(|item| format!("<li>{}</li>", escape_html(&item)))
        .collect::<String>();
    let checks = meta
        .next_checks
        .iter()
        .map(|check| format!("<li>{}</li>", escape_html(check)))
        .collect::<String>();
    let stale_banner = if finding.display_stale {
        "<div class=\"state-banner\"><strong>Stale finding</strong>The last observation is too old to describe current state. Actions are disabled. Absence of a newer finding does not establish health.</div>"
    } else if finding.status == DashboardFindingStatus::Recovering {
        "<div class=\"state-banner\"><strong>No longer observed; confirmation is pending</strong>This is not the same as resolved or healthy. Actions are disabled while NQ confirms disappearance.</div>"
    } else if finding.status == DashboardFindingStatus::Suppressed {
        "<div class=\"state-banner\"><strong>Current observation unavailable</strong>Last-known evidence is retained, but an upstream loss of standing prevents a current claim. Actions are disabled.</div>"
    } else if finding.status == DashboardFindingStatus::Retired {
        "<div class=\"state-banner\"><strong>Evidence source retired</strong>This record is retained for audit and does not describe current health. Actions are disabled.</div>"
    } else {
        ""
    };
    let observation_at = finding
        .current_observation
        .as_ref()
        .map(|observation| observation.observed_at.as_str())
        .unwrap_or(&finding.last_seen_at);

    let content = format!(
        "<main id=\"main-content\">\
           <a class=\"back-link\" href=\"/\">← Current dashboard</a>\
           <section aria-labelledby=\"finding-heading\">\
             <div class=\"eyebrow\">Decision</div>\
             <h1 id=\"finding-heading\">{title}</h1>\
             <p class=\"observed-claim\">{message}</p>\
             {stale_banner}\
             <dl class=\"decision-facts\">\
               <div><dt>State</dt><dd>{status}</dd></div>\
               <div><dt>Affected</dt><dd>{host}{subject}</dd></div>\
               <div><dt>Observed</dt><dd><time datetime=\"{at}\">{at}</time> ({age} ago)</dd></div>\
               <div><dt>Comparison / magnitude</dt><dd>{magnitude}</dd></div>\
               <div><dt>Impact</dt><dd>{impact}</dd></div>\
               <div><dt>Coordination</dt><dd>{work_state}</dd></div>\
             </dl>\
             <div class=\"unknowns\"><strong>What remains unknown</strong><ul>{unknown_items}</ul></div>\
             <details class=\"next-inspection\" open><summary>Recommended next inspection</summary><ul>{checks}</ul></details>\
           </section>\
           <section class=\"evidence-panel\" aria-labelledby=\"evidence-heading\">\
             <div class=\"eyebrow\">Evidence</div><h2 id=\"evidence-heading\">Why NQ reports this</h2>\
             {evidence}\
             <details><summary>Retained observation history ({observation_count})</summary>{observations}</details>\
           </section>\
           <section class=\"action-panel\" aria-labelledby=\"action-heading\">\
             <div class=\"eyebrow\">Available next action</div><h2 id=\"action-heading\">Operator coordination</h2>\
             {actions}\
           </section>\
           <section class=\"advanced-panel\" aria-labelledby=\"advanced-heading\">\
             <div class=\"eyebrow\">Expert detail</div><h2 id=\"advanced-heading\">Epistemic and implementation record</h2>\
             <details class=\"advanced\"><summary>Classification, identity, and provenance</summary>\
               <dl>\
                 <div><dt>Operator translation</dt><dd>{domain_plain}</dd></div>\
                 <div><dt>Delta class</dt><dd><code>{domain}</code></dd></div>\
                 <div><dt>Detector</dt><dd><code>{kind}</code></dd></div>\
                 <div><dt>Stable finding identity</dt><dd><code>{key}</code></dd></div>\
                 <div><dt>Finding observation snapshot</dt><dd>#{finding_generation}</dd></div>\
                 <div><dt>Page read snapshot</dt><dd>{page_generation}</dd></div>\
                 <div><dt>Basis state</dt><dd><code>{basis_state}</code></dd></div>\
                 <div><dt>Basis source</dt><dd>{basis_source}</dd></div>\
                 <div><dt>Basis witness</dt><dd>{basis_witness}</dd></div>\
                 <div><dt>Failure class</dt><dd>{failure_class}</dd></div>\
               </dl>\
               <h3>Detector rationale, not causal fact</h3><p>{gloss}</p><p>{contradiction}</p>\
             </details>\
             <details><summary>Durable operator transition history ({transition_count})</summary>{transitions}</details>\
             <details class=\"expert-tools\"><summary>Attached expert SQL</summary>\
               <p>Use the stable finding identity in <code>finding_observations</code>; SQL is not required for the primary workflow.</p>\
               <form class=\"sql-form\" onsubmit=\"runExpertQuery(event)\">\
                 <label for=\"expert-sql\">Read-only SQL</label>\
                 <textarea id=\"expert-sql\" name=\"sql\" rows=\"3\">SELECT generation_id, observed_at, value, message FROM finding_observations WHERE finding_key = '{sql_key}' ORDER BY generation_id DESC</textarea>\
                 <button type=\"submit\">Run read-only query</button>\
               </form><pre id=\"expert-result\" aria-live=\"polite\"></pre>\
             </details>\
           </section>\
         </main>",
        title = finding_title(finding),
        message = escape_html(&finding.message),
        stale_banner = stale_banner,
        status = escape_html(status_label(finding.status)),
        host = escape_html(&finding.host),
        subject = if finding.subject.is_empty() {
            String::new()
        } else {
            format!(" / {}", escape_html(&finding.subject))
        },
        at = escape_html(observation_at),
        age = finding
            .observation_age_seconds
            .map(human_duration)
            .unwrap_or_else(|| "unknown".into()),
        magnitude = match detail.evidence.as_ref() {
            Some(DashboardEvidence::ErrorShift(evidence)) => format!(
                "{} now versus {} baseline; {} current messages",
                percentage(evidence.current_error_ratio),
                percentage(evidence.baseline_average_error_ratio),
                evidence.current_sample_size
            ),
            None => optional_number(finding.peak_value, ""),
        },
        impact = escape_html(&impact_statement(finding)),
        work_state = escape_html(&finding.work_state),
        unknown_items = unknown_items,
        checks = checks,
        evidence = render_evidence(finding, detail.evidence.as_ref()),
        observation_count = detail.observations.total_count,
        observations = render_observation_history(&detail.observations),
        actions = render_actions(finding, &detail.basis, mutation_available),
        domain_plain = escape_html(plain_domain(&finding.domain)),
        domain = escape_html(&finding.domain),
        kind = escape_html(&finding.kind),
        key = escape_html(&finding.finding_key),
        finding_generation = finding.last_seen_generation,
        page_generation = detail
            .basis
            .generation_id
            .map(|value| format!("#{value}"))
            .unwrap_or_else(|| "Unavailable".into()),
        basis_state = escape_html(&finding.basis_state),
        basis_source = optional_text(finding.basis_source_id.as_deref()),
        basis_witness = optional_text(finding.basis_witness_id.as_deref()),
        failure_class = optional_text(finding.diagnosis.failure_class.as_deref()),
        gloss = escape_html(meta.gloss),
        contradiction = escape_html(meta.contradiction),
        transition_count = detail.transitions.total_count,
        transitions = render_transition_history(&detail.transitions),
        sql_key = finding.finding_key.replace('\'', "''"),
    );
    page_shell(&meta.plain_label, &detail.basis, content)
}

fn render_historical_detail(detail: &DashboardHistoricalFindingDetail) -> String {
    let identity = &detail.identity;
    let meta = nq_db::finding_meta::finding_meta(&identity.kind);
    let latest = detail.latest_observation.as_ref();
    let content = format!(
        "<main id=\"main-content\">\
           <a class=\"back-link\" href=\"/\">← Current dashboard</a>\
           <section class=\"history-state\" aria-labelledby=\"historical-heading\">\
             <div class=\"eyebrow\">Historical record</div><h1 id=\"historical-heading\">{title}</h1>\
             <div class=\"state-banner\"><strong>This finding is no longer in current lifecycle state.</strong>\
               Its observations and coordination history are retained. NQ cannot infer from disappearance alone whether the condition resolved, expired, was superseded, or was removed. It does not establish current health.</div>\
             <dl class=\"decision-facts\">\
               <div><dt>Affected</dt><dd>{host}{subject}</dd></div>\
               <div><dt>Last retained observation</dt><dd>{latest_at}</dd></div>\
               <div><dt>Last retained value</dt><dd>{latest_value}</dd></div>\
             </dl>\
             <p class=\"action-unavailable\"><strong>No mutation target is active.</strong> Historical evidence cannot be acknowledged, suppressed, closed, or reset.</p>\
           </section>\
           <section class=\"evidence-panel\" aria-labelledby=\"historical-evidence-heading\">\
             <div class=\"eyebrow\">Evidence</div><h2 id=\"historical-evidence-heading\">Retained observations</h2>\
             {observations}\
           </section>\
           <section class=\"advanced-panel\" aria-labelledby=\"historical-advanced-heading\">\
             <div class=\"eyebrow\">Expert detail</div><h2 id=\"historical-advanced-heading\">Identity and durable transitions</h2>\
             <details class=\"advanced\"><summary>Classification and stable identity</summary><dl>\
               <div><dt>Detector</dt><dd><code>{kind}</code></dd></div>\
               <div><dt>Delta class</dt><dd>{domain}</dd></div>\
               <div><dt>Stable finding identity</dt><dd><code>{key}</code></dd></div>\
             </dl></details>\
             {transitions}\
           </section>\
         </main>",
        title = escape_html(meta.plain_label),
        host = escape_html(&identity.host),
        subject = if identity.subject.is_empty() {
            String::new()
        } else {
            format!(" / {}", escape_html(&identity.subject))
        },
        latest_at = latest
            .map(|observation| {
                format!(
                    "<time datetime=\"{0}\">{0}</time> (historical)",
                    escape_html(&observation.observed_at)
                )
            })
            .unwrap_or_else(|| {
                "<span class=\"unknown-value\">No retained observation time</span>".into()
            }),
        latest_value = latest
            .and_then(|observation| observation.value)
            .map(|value| escape_html(&value.to_string()))
            .unwrap_or_else(|| "<span class=\"unknown-value\">Unavailable</span>".into()),
        observations = render_observation_history(&detail.observations),
        kind = escape_html(&identity.kind),
        domain = identity
            .domain
            .as_deref()
            .map(|domain| {
                format!(
                    "{} · <code>{}</code>",
                    escape_html(plain_domain(domain)),
                    escape_html(domain)
                )
            })
            .unwrap_or_else(|| "<span class=\"unknown-value\">Unavailable</span>".into()),
        key = escape_html(&identity.finding_key),
        transitions = render_transition_history(&detail.transitions),
    );
    page_shell("Historical finding", &detail.basis, content)
}

fn render_missing_detail(detail: &DashboardMissingFindingDetail) -> String {
    let content = format!(
        "<main id=\"main-content\">\
           <a class=\"back-link\" href=\"/\">← Current dashboard</a>\
           <section class=\"missing-state\" aria-labelledby=\"missing-heading\">\
             <div class=\"eyebrow\">Finding unavailable</div><h1 id=\"missing-heading\">Finding cannot be resolved</h1>\
             <p>NQ could not resolve the requested stable identity in current or retained history.</p>\
             <dl><div><dt>Requested finding</dt><dd><code>{key}</code></dd></div></dl>\
             <div class=\"unknowns\"><strong>What remains unknown</strong><ul>\
               <li>Whether the underlying condition is healthy, unhealthy, resolved, expired, superseded, deleted, or was never recorded here.</li>\
               <li>Whether a different retained finding identity describes the same operational subject.</li>\
             </ul></div>\
             <p><strong>No mutation controls are available.</strong> There is no unambiguous current target. Previously viewed finding content is not retained on this route.</p>\
             <p><a class=\"primary-link\" href=\"/\">Inspect current issues and retained inventory</a></p>\
           </section>\
         </main>",
        key = escape_html(&detail.requested_finding_key),
    );
    page_shell("Finding unavailable", &detail.basis, content)
}

pub fn render_finding_detail(detail: &DashboardFindingDetail, mutation_available: bool) -> String {
    match detail {
        DashboardFindingDetail::Current(detail) => {
            render_current_detail(detail, mutation_available)
        }
        DashboardFindingDetail::Historical(detail) => render_historical_detail(detail),
        DashboardFindingDetail::Missing(detail) => render_missing_detail(detail),
    }
}

pub fn render_load_failure(message: &str) -> String {
    let basis = DashboardBasis {
        generation_id: None,
        completed_at: None,
        status: None,
        age_seconds: None,
        loaded_at: String::new(),
    };
    let content = format!(
        "<main id=\"main-content\"><section class=\"missing-state\" aria-labelledby=\"load-heading\">\
         <div class=\"eyebrow\">Dashboard unavailable</div><h1 id=\"load-heading\">NQ could not load a coherent observation snapshot</h1>\
         <p>No current-health conclusion or mutation target is shown.</p>\
         <details><summary>Storage error</summary><pre>{}</pre></details></section></main>",
        escape_html(message)
    );
    page_shell("Dashboard unavailable", &basis, content)
}
