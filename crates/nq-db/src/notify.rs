//! Notification engine: detect severity escalations and emit alerts.
//!
//! After each generation's detector + lifecycle cycle, check which findings
//! have escalated to a severity that hasn't been notified yet. Produce
//! notification payloads for those findings, then mark them as notified.
//!
//! Notification history persists across warning_state row deletion so that
//! cyclical conditions are labeled (recurring) rather than (new).

use crate::WriteDb;

/// How a notification relates to prior notifications for the same identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationKind {
    /// Never notified about this (host, kind, subject) before.
    New,
    /// Notified before, but outside the cooldown window.
    Recurring { last_severity: String },
    /// Current severity is higher than what was last notified.
    Escalated { from_severity: String },
}

/// A finding that needs notification because its severity escalated.
#[derive(Debug, Clone)]
pub struct PendingNotification {
    pub host: String,
    pub domain: String,
    pub kind: String,
    pub subject: String,
    pub message: String,
    pub severity: String,
    pub notification_kind: NotificationKind,
    pub consecutive_gens: i64,
    pub first_seen_at: String,
    pub peak_value: Option<f64>,
}

// Cooldown: don't re-announce same identity as (new) within this window.
const COOLDOWN_HOURS: i64 = 24;

/// Find findings whose severity exceeds their last notified severity.
/// Only returns findings at or above `min_severity`.
pub fn find_pending(db: &WriteDb, min_severity: &str) -> anyhow::Result<Vec<PendingNotification>> {
    let min_rank = severity_rank(min_severity);

    let mut stmt = db.conn.prepare(
        "SELECT ws.host, ws.domain, ws.kind, ws.subject, ws.message, ws.severity,
                ws.notified_severity, ws.consecutive_gens, ws.first_seen_at, ws.peak_value,
                nh.last_notified_at, nh.last_notified_severity, nh.notification_count
         FROM warning_state ws
         LEFT JOIN notification_history nh
           ON ws.host = nh.host AND ws.kind = nh.kind AND ws.subject = nh.subject
         WHERE (ws.notified_severity IS NULL OR ws.severity != ws.notified_severity)
           AND COALESCE(ws.work_state, 'new') NOT IN ('quiesced', 'suppressed', 'closed')",
    )?;

    let now = time::OffsetDateTime::now_utc();

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,   // host
            row.get::<_, String>(1)?,   // domain
            row.get::<_, String>(2)?,   // kind
            row.get::<_, String>(3)?,   // subject
            row.get::<_, String>(4)?,   // message
            row.get::<_, String>(5)?,   // severity
            row.get::<_, Option<String>>(6)?,  // notified_severity (from warning_state)
            row.get::<_, i64>(7)?,      // consecutive_gens
            row.get::<_, String>(8)?,   // first_seen_at
            row.get::<_, Option<f64>>(9)?, // peak_value
            row.get::<_, Option<String>>(10)?, // nh.last_notified_at
            row.get::<_, Option<String>>(11)?, // nh.last_notified_severity
            row.get::<_, Option<i64>>(12)?,    // nh.notification_count
        ))
    })?;

    let mut pending = Vec::new();
    for row in rows {
        let (host, domain, kind, subject, message, severity,
             _notified_sev, gens, first_seen, peak,
             hist_last_at, hist_last_sev, _hist_count) = row?;

        let current_rank = severity_rank(&severity);

        // Determine notification kind using durable history
        let notification_kind = match (&hist_last_at, &hist_last_sev) {
            (Some(last_at_str), Some(last_sev)) => {
                let last_sev_rank = severity_rank(last_sev);

                if current_rank > last_sev_rank {
                    // Genuine escalation: severity increased beyond what we last notified
                    NotificationKind::Escalated { from_severity: last_sev.clone() }
                } else {
                    // Same or lower severity — check cooldown
                    let within_cooldown = parse_rfc3339(last_at_str)
                        .map(|last_at| {
                            let elapsed = now - last_at;
                            elapsed.whole_hours() < COOLDOWN_HOURS
                        })
                        .unwrap_or(false);

                    if within_cooldown {
                        // Suppress: we already notified about this recently
                        continue;
                    }

                    NotificationKind::Recurring { last_severity: last_sev.clone() }
                }
            }
            _ => {
                // No history at all — genuinely new
                NotificationKind::New
            }
        };

        if current_rank >= min_rank {
            pending.push(PendingNotification {
                host,
                domain,
                kind,
                subject,
                message,
                severity,
                notification_kind,
                consecutive_gens: gens,
                first_seen_at: first_seen,
                peak_value: peak,
            });
        }
    }

    Ok(pending)
}

/// Mark a finding as notified at its current severity. Writes through to
/// both warning_state (for in-lifecycle tracking) and notification_history
/// (for durable cross-lifecycle memory).
pub fn mark_notified(db: &mut WriteDb, host: &str, kind: &str, subject: &str, severity: &str) -> anyhow::Result<()> {
    let now = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .expect("timestamp format");

    let dedup_key = format!("{}:{}:{}:{}", host, kind, subject, severity);

    // Update warning_state (may be absent if row was already GC'd, that's fine)
    db.conn.execute(
        "UPDATE warning_state SET notified_severity = ?1, notified_at = ?2, last_notification_dedup_key = ?3
         WHERE host = ?4 AND kind = ?5 AND subject = ?6",
        rusqlite::params![severity, &now, &dedup_key, host, kind, subject],
    )?;

    // Write-through to durable notification_history
    db.conn.execute(
        "INSERT INTO notification_history (host, kind, subject, first_notified_at, last_notified_at, last_notified_severity, notification_count)
         VALUES (?1, ?2, ?3, ?4, ?4, ?5, 1)
         ON CONFLICT(host, kind, subject) DO UPDATE SET
             last_notified_at = ?4,
             last_notified_severity = ?5,
             notification_count = notification_count + 1",
        rusqlite::params![host, kind, subject, &now, severity],
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

fn escalation_label(nk: &NotificationKind) -> String {
    match nk {
        NotificationKind::New => " (new)".to_string(),
        NotificationKind::Recurring { last_severity } => format!(" (recurring, prev {})", last_severity),
        NotificationKind::Escalated { from_severity } => format!(" (escalated from {})", from_severity),
    }
}

/// Build a JSON payload for a webhook notification.
pub fn build_webhook_payload(n: &PendingNotification, generation_id: i64, base_url: &str) -> serde_json::Value {
    let domain_label = domain_label(&n.domain);

    let (previous_severity, is_recurring) = match &n.notification_kind {
        NotificationKind::New => (None, false),
        NotificationKind::Recurring { last_severity } => (Some(last_severity.as_str()), true),
        NotificationKind::Escalated { from_severity } => (Some(from_severity.as_str()), false),
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
            "previous_severity": previous_severity,
            "is_recurring": is_recurring,
            "consecutive_gens": n.consecutive_gens,
            "first_seen_at": n.first_seen_at,
            "peak_value": n.peak_value,
        }
    })
}

/// Build a Slack message payload.
pub fn build_slack_payload(n: &PendingNotification, generation_id: i64, base_url: &str) -> serde_json::Value {
    let domain_label = domain_label(&n.domain);

    let emoji = match n.severity.as_str() {
        "critical" => ":red_circle:",
        "warning" => ":large_orange_circle:",
        _ => ":white_circle:",
    };

    let escalation = escalation_label(&n.notification_kind);
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
    let domain_label = domain_label(&n.domain);

    let emoji = match n.severity.as_str() {
        "critical" => "\u{1F534}", // red circle
        "warning" => "\u{1F7E0}",  // orange circle
        _ => "\u{26AA}",           // white circle
    };

    let escalation = escalation_label(&n.notification_kind);
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
        &n.first_seen_at[..19.min(n.first_seen_at.len())],
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

fn domain_label(domain: &str) -> &str {
    match domain {
        "Δo" => "missing",
        "Δs" => "skewed",
        "Δg" => "unstable",
        "Δh" => "degrading",
        other => other,
    }
}

fn parse_rfc3339(s: &str) -> Option<time::OffsetDateTime> {
    time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migrate, open_rw};

    fn setup_db() -> crate::WriteDb {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.into_path().join("test.db");
        let mut db = open_rw(&db_path).unwrap();
        migrate(&mut db).unwrap();
        db
    }

    fn insert_finding(db: &mut crate::WriteDb, host: &str, kind: &str, subject: &str, severity: &str) {
        let now = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();
        db.conn.execute(
            "INSERT OR REPLACE INTO warning_state (host, kind, subject, domain, message, severity,
                first_seen_gen, first_seen_at, last_seen_gen, last_seen_at, consecutive_gens, absent_gens)
             VALUES (?1, ?2, ?3, 'Δg', 'test message', ?4, 1, ?5, 1, ?5, 31, 0)",
            rusqlite::params![host, kind, subject, severity, &now],
        ).unwrap();
    }

    #[test]
    fn new_finding_labeled_new() {
        let mut db = setup_db();
        insert_finding(&mut db, "host1", "wal_bloat", "/tmp/test.db", "warning");

        let pending = find_pending(&db, "info").unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].notification_kind, NotificationKind::New);
    }

    #[test]
    fn after_notify_no_duplicate() {
        let mut db = setup_db();
        insert_finding(&mut db, "host1", "wal_bloat", "/tmp/test.db", "warning");

        // Notify
        mark_notified(&mut db, "host1", "wal_bloat", "/tmp/test.db", "warning").unwrap();

        // Should be empty — warning_state.notified_severity matches severity
        let pending = find_pending(&db, "info").unwrap();
        assert!(pending.is_empty());
    }

    #[test]
    fn escalation_detected() {
        let mut db = setup_db();
        insert_finding(&mut db, "host1", "wal_bloat", "/tmp/test.db", "warning");
        mark_notified(&mut db, "host1", "wal_bloat", "/tmp/test.db", "warning").unwrap();

        // Escalate to critical
        db.conn.execute(
            "UPDATE warning_state SET severity = 'critical' WHERE host = 'host1' AND kind = 'wal_bloat'",
            [],
        ).unwrap();

        let pending = find_pending(&db, "info").unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].notification_kind, NotificationKind::Escalated {
            from_severity: "warning".to_string(),
        });
    }

    #[test]
    fn flap_after_row_deletion_labeled_recurring() {
        let mut db = setup_db();

        // First occurrence: notify
        insert_finding(&mut db, "host1", "wal_bloat", "/tmp/test.db", "warning");
        mark_notified(&mut db, "host1", "wal_bloat", "/tmp/test.db", "warning").unwrap();

        // Simulate row deletion (recovery window exceeded)
        db.conn.execute(
            "DELETE FROM warning_state WHERE host = 'host1' AND kind = 'wal_bloat'",
            [],
        ).unwrap();

        // Backdate the notification_history so it's outside cooldown
        db.conn.execute(
            "UPDATE notification_history SET last_notified_at = '2020-01-01T00:00:00Z'
             WHERE host = 'host1' AND kind = 'wal_bloat'",
            [],
        ).unwrap();

        // Finding reappears — fresh warning_state row, no notified_severity
        insert_finding(&mut db, "host1", "wal_bloat", "/tmp/test.db", "warning");

        let pending = find_pending(&db, "info").unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].notification_kind, NotificationKind::Recurring {
            last_severity: "warning".to_string(),
        });
    }

    #[test]
    fn flap_within_cooldown_suppressed() {
        let mut db = setup_db();

        // First occurrence: notify
        insert_finding(&mut db, "host1", "wal_bloat", "/tmp/test.db", "warning");
        mark_notified(&mut db, "host1", "wal_bloat", "/tmp/test.db", "warning").unwrap();

        // Simulate row deletion
        db.conn.execute(
            "DELETE FROM warning_state WHERE host = 'host1' AND kind = 'wal_bloat'",
            [],
        ).unwrap();

        // Finding reappears (notification_history.last_notified_at is recent)
        insert_finding(&mut db, "host1", "wal_bloat", "/tmp/test.db", "warning");

        // Should be suppressed — within 24h cooldown
        let pending = find_pending(&db, "info").unwrap();
        assert!(pending.is_empty());
    }

    #[test]
    fn escalation_pierces_cooldown() {
        let mut db = setup_db();

        // First occurrence at warning: notify
        insert_finding(&mut db, "host1", "wal_bloat", "/tmp/test.db", "warning");
        mark_notified(&mut db, "host1", "wal_bloat", "/tmp/test.db", "warning").unwrap();

        // Simulate row deletion + reappearance at critical
        db.conn.execute(
            "DELETE FROM warning_state WHERE host = 'host1' AND kind = 'wal_bloat'",
            [],
        ).unwrap();
        insert_finding(&mut db, "host1", "wal_bloat", "/tmp/test.db", "critical");

        // Should notify as escalation even within cooldown
        let pending = find_pending(&db, "info").unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].notification_kind, NotificationKind::Escalated {
            from_severity: "warning".to_string(),
        });
    }

    #[test]
    fn notification_count_increments() {
        let mut db = setup_db();
        insert_finding(&mut db, "host1", "wal_bloat", "/tmp/test.db", "warning");

        mark_notified(&mut db, "host1", "wal_bloat", "/tmp/test.db", "warning").unwrap();
        mark_notified(&mut db, "host1", "wal_bloat", "/tmp/test.db", "warning").unwrap();
        mark_notified(&mut db, "host1", "wal_bloat", "/tmp/test.db", "warning").unwrap();

        let count: i64 = db.conn.query_row(
            "SELECT notification_count FROM notification_history WHERE host = 'host1' AND kind = 'wal_bloat'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn history_survives_warning_state_deletion() {
        let mut db = setup_db();
        insert_finding(&mut db, "host1", "wal_bloat", "/tmp/test.db", "warning");
        mark_notified(&mut db, "host1", "wal_bloat", "/tmp/test.db", "warning").unwrap();

        // Delete warning_state row
        db.conn.execute("DELETE FROM warning_state WHERE host = 'host1'", []).unwrap();

        // History still there
        let count: i64 = db.conn.query_row(
            "SELECT notification_count FROM notification_history WHERE host = 'host1' AND kind = 'wal_bloat'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 1);
    }
}
