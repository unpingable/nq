//! Notification engine: detect severity escalations and emit alerts.
//!
//! After each generation's detector + lifecycle cycle, check which findings
//! have escalated to a severity that hasn't been notified yet. Produce
//! notification payloads for those findings, then mark them as notified.

use crate::WriteDb;

/// A finding that needs notification because its severity escalated.
#[derive(Debug, Clone)]
pub struct PendingNotification {
    pub host: String,
    pub domain: String,
    pub kind: String,
    pub subject: String,
    pub message: String,
    pub severity: String,
    pub previous_severity: Option<String>,
    pub consecutive_gens: i64,
    pub first_seen_at: String,
    pub peak_value: Option<f64>,
}

/// Find findings whose severity exceeds their last notified severity.
/// Only returns findings at or above `min_severity`.
pub fn find_pending(db: &WriteDb, min_severity: &str) -> anyhow::Result<Vec<PendingNotification>> {
    let min_rank = severity_rank(min_severity);

    let mut stmt = db.conn.prepare(
        "SELECT host, domain, kind, subject, message, severity,
                notified_severity, consecutive_gens, first_seen_at, peak_value
         FROM warning_state
         WHERE (notified_severity IS NULL OR severity != notified_severity)
           AND COALESCE(work_state, 'new') NOT IN ('quiesced', 'suppressed', 'closed')",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, i64>(7)?,
            row.get::<_, String>(8)?,
            row.get::<_, Option<f64>>(9)?,
        ))
    })?;

    let mut pending = Vec::new();
    for row in rows {
        let (host, domain, kind, subject, message, severity, notified_sev, gens, first_seen, peak) = row?;

        let current_rank = severity_rank(&severity);
        let notified_rank = notified_sev.as_deref().map(severity_rank).unwrap_or(0);

        // Only notify if current severity is above min threshold
        // AND current severity is higher than what we last notified
        if current_rank >= min_rank && current_rank > notified_rank {
            pending.push(PendingNotification {
                host,
                domain,
                kind,
                subject,
                message,
                severity,
                previous_severity: notified_sev,
                consecutive_gens: gens,
                first_seen_at: first_seen,
                peak_value: peak,
            });
        }
    }

    Ok(pending)
}

/// Mark a finding as notified at its current severity.
pub fn mark_notified(db: &mut WriteDb, host: &str, kind: &str, subject: &str, severity: &str) -> anyhow::Result<()> {
    let now = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .expect("timestamp format");

    db.conn.execute(
        "UPDATE warning_state SET notified_severity = ?1, notified_at = ?2
         WHERE host = ?3 AND kind = ?4 AND subject = ?5",
        rusqlite::params![severity, &now, host, kind, subject],
    )?;

    Ok(())
}

/// Build a finding detail URL.
fn finding_url(base_url: &str, n: &PendingNotification) -> String {
    let encode = |s: &str| -> String {
        s.bytes().map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                String::from(b as char)
            }
            _ => format!("%{:02X}", b),
        }).collect()
    };
    if n.subject.is_empty() {
        format!("{}/finding/{}/{}", base_url.trim_end_matches('/'), encode(&n.kind), encode(&n.host))
    } else {
        format!("{}/finding/{}/{}/{}", base_url.trim_end_matches('/'), encode(&n.kind), encode(&n.host), encode(&n.subject))
    }
}

/// Build a JSON payload for a webhook notification.
pub fn build_webhook_payload(n: &PendingNotification, generation_id: i64, base_url: &str) -> serde_json::Value {
    let domain_label = match n.domain.as_str() {
        "Δo" => "missing",
        "Δs" => "skewed",
        "Δg" => "unstable",
        "Δh" => "degrading",
        other => other,
    };

    serde_json::json!({
        "version": "nq/v1",
        "generation_id": generation_id,
        "url": finding_url(base_url, n),
        "finding": {
            "host": n.host,
            "domain": n.domain,
            "domain_label": domain_label,
            "kind": n.kind,
            "subject": n.subject,
            "message": n.message,
            "severity": n.severity,
            "previous_severity": n.previous_severity,
            "consecutive_gens": n.consecutive_gens,
            "first_seen_at": n.first_seen_at,
            "peak_value": n.peak_value,
        }
    })
}

/// Build a Slack message payload.
pub fn build_slack_payload(n: &PendingNotification, generation_id: i64, base_url: &str) -> serde_json::Value {
    let domain_label = match n.domain.as_str() {
        "Δo" => "missing",
        "Δs" => "skewed",
        "Δg" => "unstable",
        "Δh" => "degrading",
        other => other,
    };

    let emoji = match n.severity.as_str() {
        "critical" => ":red_circle:",
        "warning" => ":large_orange_circle:",
        _ => ":white_circle:",
    };

    let escalation = match &n.previous_severity {
        Some(prev) => format!(" (escalated from {})", prev),
        None => " (new)".to_string(),
    };

    let url = finding_url(base_url, n);

    let text = format!(
        "{} *<{}|[{} {}]>* {} `{}`/`{}` on *{}*\n>{}\n_gen #{} · {} consecutive · since {}_",
        emoji,
        url,
        n.severity.to_uppercase(),
        domain_label,
        escalation,
        n.kind,
        if n.subject.is_empty() { "-" } else { &n.subject },
        if n.host.is_empty() { "global" } else { &n.host },
        n.message,
        generation_id,
        n.consecutive_gens,
        n.first_seen_at,
    );

    serde_json::json!({ "text": text })
}

/// Build a Discord message payload (uses `content` not `text`).
pub fn build_discord_payload(n: &PendingNotification, generation_id: i64, base_url: &str) -> serde_json::Value {
    let domain_label = match n.domain.as_str() {
        "Δo" => "missing",
        "Δs" => "skewed",
        "Δg" => "unstable",
        "Δh" => "degrading",
        other => other,
    };

    let emoji = match n.severity.as_str() {
        "critical" => "\u{1F534}", // red circle
        "warning" => "\u{1F7E0}",  // orange circle
        _ => "\u{26AA}",           // white circle
    };

    let escalation = match &n.previous_severity {
        Some(prev) => format!(" (escalated from {})", prev),
        None => " (new)".to_string(),
    };

    let url = finding_url(base_url, n);

    let content = format!(
        "{} **[{} {}]**{} `{}`/`{}` on **{}**\n> {}\n-# gen #{} · {} consecutive · since {} · [detail]({})",
        emoji,
        n.severity.to_uppercase(),
        domain_label,
        escalation,
        n.kind,
        if n.subject.is_empty() { "-" } else { &n.subject },
        if n.host.is_empty() { "global" } else { &n.host },
        n.message,
        generation_id,
        n.consecutive_gens,
        &n.first_seen_at[..19], // trim to seconds
        url,
    );

    serde_json::json!({ "content": content })
}

fn severity_rank(s: &str) -> u8 {
    match s {
        "info" => 1,
        "warning" => 2,
        "critical" => 3,
        _ => 0,
    }
}
