//! Notification engine: detect severity escalations and emit alerts.
//!
//! After each generation's detector + lifecycle cycle, check which findings
//! have escalated to a severity that hasn't been notified yet. Produce
//! notification payloads for those findings, then mark them as notified.
//!
//! Notification history persists across warning_state row deletion so that
//! cyclical conditions are labeled (recurring) rather than (new).

use crate::detect::StateKind;
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
    /// Regime badge + explanation populated from the regime-features
    /// store at `find_pending` time. `None` means no regime signal
    /// strong enough to report — payload builders emit nothing for
    /// this case rather than a literal "none" token.
    pub regime: Option<(crate::regime::RegimeBadge, String)>,
    /// Categorical kind declared by the emitting detector. Read from
    /// warning_state.state_kind. Pre-migration rows read as
    /// `LegacyUnclassified`. Drives lane-ordered rollup rendering.
    pub state_kind: StateKind,
}

/// A group of findings that render as one operator-facing card.
///
/// Grouping key: `(host, state_kind, detector_family)`. Lane ordering
/// privileges `state_kind` first (incident > degradation > maintenance >
/// informational > legacy_unclassified); severity sorts within a lane.
///
/// Legacy-unclassified findings get their own rollup per finding — they
/// are not mixed into "clean" rollups. See ALERT_INTERPRETATION_GAP
/// §"State kind as a first-class axis" §"Migration contract".
#[derive(Debug, Clone)]
pub struct NotificationRollup {
    pub host: String,
    pub state_kind: StateKind,
    pub detector_family: String,
    /// Findings inside this rollup, sorted by severity (critical first)
    /// then by stable tiebreak (kind, subject).
    pub findings: Vec<PendingNotification>,
}

/// Detector family grouping used for rollup aggregation. Derived from
/// `kind` at rollup time. Deliberately coarse — the family is a rendering
/// grouping, not semantics. If a family designation gets contentious,
/// split the detector, don't split the family.
pub fn detector_family(kind: &str) -> &'static str {
    match kind {
        "wal_bloat" | "freelist_bloat" => "sqlite",
        k if k.starts_with("zfs_") => "zfs",
        "disk_pressure" | "mem_pressure" | "resource_drift" | "stale_host" => "host",
        "stale_service" | "service_status" | "service_flap" | "signal_dropout" => "service",
        "log_silence" | "error_shift" => "logs",
        "metric_signal" | "scrape_regime_shift" => "metric",
        "check_failed" | "check_error" => "saved_check",
        "source_error" => "source",
        _ => "other",
    }
}

/// Group pending notifications into rollups keyed by
/// `(host, state_kind, detector_family)`. `legacy_unclassified` findings
/// get their own per-finding rollup — they are not merged with kind-clean
/// groups.
///
/// Output ordering: lane order first (incident → legacy), then within-lane
/// by host / detector_family for stable rendering.
pub fn rollup_pending(pending: Vec<PendingNotification>) -> Vec<NotificationRollup> {
    use std::collections::BTreeMap;

    // Key includes an ordinal for stable lane-first ordering. For
    // legacy_unclassified we append the finding identity so each stays its
    // own rollup rather than merging with siblings — the migration contract
    // forbids clean aggregation of legacy rows.
    let mut buckets: BTreeMap<(u8, String, String, String, String), Vec<PendingNotification>> = BTreeMap::new();

    for n in pending {
        let family = detector_family(&n.kind).to_string();
        let lane = n.state_kind.lane_order();
        let kind_key = n.state_kind.as_str().to_string();
        let dedup = match n.state_kind {
            StateKind::LegacyUnclassified => format!("{}:{}:{}", n.kind, n.subject, n.host),
            _ => String::new(),
        };
        let key = (lane, kind_key, n.host.clone(), family, dedup);
        buckets.entry(key).or_default().push(n);
    }

    let mut rollups = Vec::with_capacity(buckets.len());
    for ((_lane, _kind_key, host, family, _dedup), mut findings) in buckets {
        // Severity within lane: critical first. Stable tiebreak on kind/subject.
        findings.sort_by(|a, b| {
            severity_rank(&b.severity)
                .cmp(&severity_rank(&a.severity))
                .then_with(|| a.kind.cmp(&b.kind))
                .then_with(|| a.subject.cmp(&b.subject))
        });
        let state_kind = findings
            .first()
            .map(|f| f.state_kind)
            .unwrap_or(StateKind::LegacyUnclassified);
        rollups.push(NotificationRollup {
            host,
            state_kind,
            detector_family: family,
            findings,
        });
    }
    rollups
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
                nh.last_notified_at, nh.last_notified_severity, nh.notification_count,
                ws.state_kind
         FROM warning_state ws
         LEFT JOIN notification_history nh
           ON ws.host = nh.host AND ws.kind = nh.kind AND ws.subject = nh.subject
         WHERE (ws.notified_severity IS NULL OR ws.severity != ws.notified_severity)
           AND COALESCE(ws.work_state, 'new') NOT IN ('quiesced', 'suppressed', 'closed')
           AND ws.visibility_state = 'observed'",
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
            row.get::<_, String>(13)?,         // ws.state_kind
        ))
    })?;

    let mut pending = Vec::new();
    for row in rows {
        let (host, domain, kind, subject, message, severity,
             _notified_sev, gens, first_seen, peak,
             hist_last_at, hist_last_sev, _hist_count, state_kind_str) = row?;
        let state_kind = StateKind::from_str(&state_kind_str)
            .unwrap_or(StateKind::LegacyUnclassified);

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
            let regime = crate::regime::compute_regime_annotation(&db.conn, &host, &kind, &subject)
                .unwrap_or(None);
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
                regime,
                state_kind,
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

/// Render the metadata line (escalation/generation/consecutive) in the order
/// the spec requires: escalation leads when present, otherwise generation leads.
fn format_metadata_line(nk: &NotificationKind, generation_id: i64, consecutive: i64) -> String {
    match nk {
        NotificationKind::New => format!(
            "Generation #{} · {} consecutive (new)",
            generation_id, consecutive,
        ),
        NotificationKind::Recurring { last_severity } => format!(
            "Generation #{} · {} consecutive · recurring (prev {})",
            generation_id, consecutive, last_severity,
        ),
        NotificationKind::Escalated { from_severity } => format!(
            "Escalated from {} · generation #{} · {} consecutive",
            from_severity, generation_id, consecutive,
        ),
    }
}

/// Render one finding's kind/subject as a human-legible bullet.
fn format_finding_line(kind: &str, subject: &str) -> String {
    if subject.is_empty() {
        format!("• `{}`", kind)
    } else {
        format!("• `{}` on `{}`", kind, subject)
    }
}

/// Render the "since" timestamp in operator-legible form.
/// Full-precision RFC3339 stays in the structured payload and in the evidence footer.
/// The human body gets YYYY-MM-DD HH:MM UTC plus an approximate relative age
/// when the input is in the past.
fn format_since(rfc3339_str: &str, now: time::OffsetDateTime) -> String {
    let Some(then) = parse_rfc3339(rfc3339_str) else {
        return rfc3339_str.to_string();
    };
    let absolute = format!(
        "{:04}-{:02}-{:02} {:02}:{:02} UTC",
        then.year(),
        u8::from(then.month()),
        then.day(),
        then.hour(),
        then.minute(),
    );
    if then > now {
        return absolute;
    }
    let secs = (now - then).whole_seconds();
    let relative = if secs < 60 {
        "just now".to_string()
    } else if secs < 3600 {
        format!("~{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("~{}h ago", secs / 3600)
    } else {
        format!("~{}d ago", secs / 86400)
    };
    format!("{} ({})", absolute, relative)
}

fn severity_emoji_slack(severity: &str) -> &'static str {
    match severity {
        "critical" => ":red_circle:",
        "warning" => ":large_orange_circle:",
        _ => ":white_circle:",
    }
}

fn severity_emoji_discord(severity: &str) -> &'static str {
    match severity {
        "critical" => "\u{1F534}", // red circle
        "warning" => "\u{1F7E0}",  // orange circle
        _ => "\u{26AA}",           // white circle
    }
}

fn scope_label(host: &str) -> &str {
    if host.is_empty() { "global" } else { host }
}

/// Build a JSON payload for a webhook notification.
pub fn build_webhook_payload(n: &PendingNotification, generation_id: i64, base_url: &str) -> serde_json::Value {
    let domain_label = domain_label(&n.domain);

    let (previous_severity, is_recurring) = match &n.notification_kind {
        NotificationKind::New => (None, false),
        NotificationKind::Recurring { last_severity } => (Some(last_severity.as_str()), true),
        NotificationKind::Escalated { from_severity } => (Some(from_severity.as_str()), false),
    };

    let (regime_badge, regime_explanation) = match &n.regime {
        Some((b, s)) => (Some(b.as_str()), Some(s.as_str())),
        None => (None, None),
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
            "regime_badge": regime_badge,
            "regime_explanation": regime_explanation,
        }
    })
}

/// Render the one-line regime annotation used by Slack and Discord
/// bodies. Returns an empty string when no regime is attached, so the
/// caller can unconditionally interpolate it into a format string
/// with a leading newline.
fn format_regime_line(n: &PendingNotification) -> String {
    match &n.regime {
        Some((badge, sentence)) => {
            format!("\n_Regime: {} — {}_", badge.as_str(), sentence)
        }
        None => String::new(),
    }
}

/// Build a Slack message payload.
///
/// Render shape (per ALERT_INTERPRETATION_GAP v1, single-finding slice):
///
/// - subject-led headline: `SEVERITY on {host or global} (domain label)`
/// - finding line as a bullet: `• kind on subject`
/// - metadata line: escalation/generation/consecutive
/// - since line: human-legible UTC time + approximate relative age
/// - evidence footer: the raw check/predicate message, demoted to blockquote
///
/// The structured payload (`build_webhook_payload`) is the machine interface.
/// The rendered text is a projection, not identity — do not parse it back.
pub fn build_slack_payload(n: &PendingNotification, generation_id: i64, base_url: &str) -> serde_json::Value {
    let now = time::OffsetDateTime::now_utc();
    let domain_label = domain_label(&n.domain);
    let emoji = severity_emoji_slack(&n.severity);
    let url = finding_url(base_url, n);
    let scope = scope_label(&n.host);
    let finding_line = format_finding_line(&n.kind, &n.subject);
    let metadata = format_metadata_line(&n.notification_kind, generation_id, n.consecutive_gens);
    let since_line = format_since(&n.first_seen_at, now);

    let regime_line = format_regime_line(n);

    let text = format!(
        "{} *<{}|{} on {}>* _({})_\n{}\n_{}_\n_Since {}_{}\n> _Source:_ {}",
        emoji,
        url,
        n.severity.to_uppercase(),
        scope,
        domain_label,
        finding_line,
        metadata,
        since_line,
        regime_line,
        n.message,
    );

    serde_json::json!({ "text": text })
}

/// Build a Discord message payload (uses `content` not `text`).
///
/// Parallel structure to the Slack payload. Uses Discord's small-text marker
/// (`-#`) for metadata lines.
pub fn build_discord_payload(n: &PendingNotification, generation_id: i64, base_url: &str) -> serde_json::Value {
    let now = time::OffsetDateTime::now_utc();
    let domain_label = domain_label(&n.domain);
    let emoji = severity_emoji_discord(&n.severity);
    let url = finding_url(base_url, n);
    let scope = scope_label(&n.host);
    let finding_line = format_finding_line(&n.kind, &n.subject);
    let metadata = format_metadata_line(&n.notification_kind, generation_id, n.consecutive_gens);
    let since_line = format_since(&n.first_seen_at, now);

    // Discord uses its small-text marker (-#) for secondary lines.
    let regime_line = match &n.regime {
        Some((badge, sentence)) => format!("\n-# Regime: {} — {}", badge.as_str(), sentence),
        None => String::new(),
    };

    let content = format!(
        "{} **{} on {}** _({})_\n{}\n-# {}\n-# Since {}{}\n-# [detail]({})\n> _Source:_ {}",
        emoji,
        n.severity.to_uppercase(),
        scope,
        domain_label,
        finding_line,
        metadata,
        since_line,
        regime_line,
        url,
        n.message,
    );

    serde_json::json!({ "content": content })
}

/// Render a rollup as a Slack payload.
///
/// Single-finding rollups delegate to `build_slack_payload` so the common
/// case is unchanged. Multi-finding rollups render one enumerated card
/// with a lane-aware headline. Maintenance lane uses backlog phrasing;
/// legacy lane carries a `[legacy]` marker.
pub fn build_rollup_slack_payload(
    r: &NotificationRollup,
    generation_id: i64,
    base_url: &str,
) -> serde_json::Value {
    if r.findings.len() == 1 {
        let mut payload = build_slack_payload(&r.findings[0], generation_id, base_url);
        if matches!(r.state_kind, StateKind::LegacyUnclassified) {
            if let Some(obj) = payload.as_object_mut() {
                if let Some(text) = obj.get_mut("text").and_then(|v| v.as_str()).map(str::to_string) {
                    obj.insert("text".into(), serde_json::Value::String(format!("{text}\n_[legacy: state_kind unclassified]_")));
                }
            }
        }
        return payload;
    }

    let now = time::OffsetDateTime::now_utc();
    let scope = scope_label(&r.host);
    let family = &r.detector_family;
    let count = r.findings.len();

    let (emoji, headline_verb) = rollup_headline_parts_slack(r.state_kind);
    let headline = match r.state_kind {
        StateKind::Maintenance => format!(
            "{} *{} {} backlog on {}* — {} finding{}",
            emoji,
            family.to_uppercase(),
            headline_verb,
            scope,
            count,
            if count == 1 { "" } else { "s" },
        ),
        StateKind::Informational => format!(
            "{} *{} {} on {}* — {} finding{}",
            emoji,
            family.to_uppercase(),
            headline_verb,
            scope,
            count,
            if count == 1 { "" } else { "s" },
        ),
        _ => format!(
            "{} *{} on {}* ({}) — {} finding{}",
            emoji,
            headline_verb,
            scope,
            family,
            count,
            if count == 1 { "" } else { "s" },
        ),
    };

    let mut bullets = String::new();
    for f in &r.findings {
        bullets.push_str(&format!("{}\n", format_finding_line(&f.kind, &f.subject)));
    }

    // Metadata and since: use the oldest finding as the anchor. Escalation
    // information collapses to "see below" when rollups mix notification
    // kinds; for v1 we render the single-case metadata if all findings
    // share it, otherwise a plain gen marker.
    let anchor = r.findings.iter()
        .min_by(|a, b| a.first_seen_at.cmp(&b.first_seen_at))
        .expect("rollup has at least one finding");
    let metadata = format_metadata_line(&anchor.notification_kind, generation_id, anchor.consecutive_gens);
    let since_line = format_since(&anchor.first_seen_at, now);

    let regime_line = format_regime_line(anchor);

    let text = format!(
        "{}\n{}_{}_\n_Since {}_{}",
        headline,
        bullets,
        metadata,
        since_line,
        regime_line,
    );

    serde_json::json!({ "text": text })
}

/// Render a rollup as a Discord payload. Mirrors the Slack shape.
pub fn build_rollup_discord_payload(
    r: &NotificationRollup,
    generation_id: i64,
    base_url: &str,
) -> serde_json::Value {
    if r.findings.len() == 1 {
        let mut payload = build_discord_payload(&r.findings[0], generation_id, base_url);
        if matches!(r.state_kind, StateKind::LegacyUnclassified) {
            if let Some(obj) = payload.as_object_mut() {
                if let Some(content) = obj.get_mut("content").and_then(|v| v.as_str()).map(str::to_string) {
                    obj.insert("content".into(), serde_json::Value::String(format!("{content}\n-# [legacy: state_kind unclassified]")));
                }
            }
        }
        return payload;
    }

    let now = time::OffsetDateTime::now_utc();
    let scope = scope_label(&r.host);
    let family = &r.detector_family;
    let count = r.findings.len();

    let (emoji, headline_verb) = rollup_headline_parts_discord(r.state_kind);
    let headline = match r.state_kind {
        StateKind::Maintenance => format!(
            "{} **{} {} backlog on {}** — {} finding{}",
            emoji, family.to_uppercase(), headline_verb, scope, count,
            if count == 1 { "" } else { "s" },
        ),
        StateKind::Informational => format!(
            "{} **{} {} on {}** — {} finding{}",
            emoji, family.to_uppercase(), headline_verb, scope, count,
            if count == 1 { "" } else { "s" },
        ),
        _ => format!(
            "{} **{} on {}** ({}) — {} finding{}",
            emoji, headline_verb, scope, family, count,
            if count == 1 { "" } else { "s" },
        ),
    };

    let mut bullets = String::new();
    for f in &r.findings {
        bullets.push_str(&format!("{}\n", format_finding_line(&f.kind, &f.subject)));
    }

    let anchor = r.findings.iter()
        .min_by(|a, b| a.first_seen_at.cmp(&b.first_seen_at))
        .expect("rollup has at least one finding");
    let metadata = format_metadata_line(&anchor.notification_kind, generation_id, anchor.consecutive_gens);
    let since_line = format_since(&anchor.first_seen_at, now);

    let regime_line = match &anchor.regime {
        Some((badge, sentence)) => format!("\n-# Regime: {} — {}", badge.as_str(), sentence),
        None => String::new(),
    };

    let content = format!(
        "{}\n{}-# {}\n-# Since {}{}",
        headline, bullets, metadata, since_line, regime_line,
    );

    serde_json::json!({ "content": content })
}

/// Render a rollup as a JSON webhook payload.
pub fn build_rollup_webhook_payload(
    r: &NotificationRollup,
    generation_id: i64,
    base_url: &str,
) -> serde_json::Value {
    let findings: Vec<_> = r.findings.iter().map(|f| {
        let (previous_severity, is_recurring) = match &f.notification_kind {
            NotificationKind::New => (None, false),
            NotificationKind::Recurring { last_severity } => (Some(last_severity.as_str()), true),
            NotificationKind::Escalated { from_severity } => (Some(from_severity.as_str()), false),
        };
        let (regime_badge, regime_explanation) = match &f.regime {
            Some((b, s)) => (Some(b.as_str()), Some(s.as_str())),
            None => (None, None),
        };
        serde_json::json!({
            "host": f.host,
            "domain": f.domain,
            "kind": f.kind,
            "subject": f.subject,
            "message": f.message,
            "severity": f.severity,
            "previous_severity": previous_severity,
            "is_recurring": is_recurring,
            "consecutive_gens": f.consecutive_gens,
            "first_seen_at": f.first_seen_at,
            "peak_value": f.peak_value,
            "regime_badge": regime_badge,
            "regime_explanation": regime_explanation,
            "url": finding_url(base_url, f),
        })
    }).collect();

    serde_json::json!({
        "version": "nq/v2",
        "rollup": true,
        "generation_id": generation_id,
        "host": r.host,
        "state_kind": r.state_kind.as_str(),
        "detector_family": r.detector_family,
        "finding_count": r.findings.len(),
        "findings": findings,
    })
}

/// Rollup emoji + verb by state_kind for Slack surfaces. Kind-first:
/// severity ranking inside a lane is preserved in `findings[0].severity`.
fn rollup_headline_parts_slack(state_kind: StateKind) -> (&'static str, &'static str) {
    match state_kind {
        StateKind::Incident => (":red_circle:", "INCIDENT"),
        StateKind::Degradation => (":large_orange_circle:", "DEGRADATION"),
        StateKind::Maintenance => (":wrench:", "maintenance"),
        StateKind::Informational => (":information_source:", "informational"),
        StateKind::LegacyUnclassified => (":grey_question:", "legacy findings"),
    }
}

fn rollup_headline_parts_discord(state_kind: StateKind) -> (&'static str, &'static str) {
    match state_kind {
        StateKind::Incident => ("\u{1F534}", "INCIDENT"),
        StateKind::Degradation => ("\u{1F7E0}", "DEGRADATION"),
        StateKind::Maintenance => ("\u{1F527}", "maintenance"),       // wrench
        StateKind::Informational => ("\u{2139}", "informational"),    // info source
        StateKind::LegacyUnclassified => ("\u{2754}", "legacy findings"),
    }
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

    // ----- Render tests (ALERT_INTERPRETATION_GAP v1, single-finding slice) -----

    fn sample_notification(
        host: &str,
        kind: &str,
        subject: &str,
        severity: &str,
        message: &str,
        nk: NotificationKind,
        consecutive: i64,
        first_seen_at: &str,
    ) -> PendingNotification {
        PendingNotification {
            host: host.into(),
            domain: "Δg".into(),
            kind: kind.into(),
            subject: subject.into(),
            message: message.into(),
            severity: severity.into(),
            notification_kind: nk,
            consecutive_gens: consecutive,
            first_seen_at: first_seen_at.into(),
            peak_value: None,
            regime: None,
            state_kind: StateKind::LegacyUnclassified,
        }
    }

    #[test]
    fn slack_render_is_subject_led_and_demotes_predicate_to_footer() {
        // The canonical 4:47 AM alert that motivated ALERT_INTERPRETATION_GAP.
        let n = sample_notification(
            "",
            "check_failed",
            "#1",
            "critical",
            "check 'critical findings': 1 row(s) (expected none)",
            NotificationKind::Escalated { from_severity: "warning".into() },
            181,
            "2026-04-14T05:45:54.549615Z",
        );

        let payload = build_slack_payload(&n, 35053, "https://nq.example");
        let text = payload["text"].as_str().unwrap();
        let headline = text.lines().next().unwrap();

        // Headline: severity-led, subject-led, no row-count predicate.
        assert!(headline.contains("CRITICAL on global"), "headline not subject-led: {}", headline);
        assert!(!headline.contains("row(s)"), "headline must not carry row-count: {}", headline);
        assert!(!headline.contains("expected none"), "headline must not carry predicate: {}", headline);

        // Finding line on its own bullet.
        assert!(text.contains("• `check_failed` on `#1`"), "finding line missing: {}", text);

        // Metadata preserved.
        assert!(text.contains("Escalated from warning"), "escalation missing: {}", text);
        assert!(text.contains("generation #35053"), "generation missing: {}", text);
        assert!(text.contains("181 consecutive"), "consecutive count missing: {}", text);

        // Human-legible time; nanoseconds stripped.
        assert!(text.contains("Since 2026-04-14 05:45 UTC"), "pretty time missing: {}", text);
        assert!(!text.contains("54.549615Z"), "nanoseconds must not leak into body: {}", text);

        // Predicate message demoted to Source footer (preserved, not erased).
        assert!(text.contains("_Source:_"), "source footer marker missing: {}", text);
        assert!(text.contains("1 row(s) (expected none)"), "raw message must survive as evidence: {}", text);
    }

    #[test]
    fn slack_render_subject_led_with_host() {
        let n = sample_notification(
            "labelwatch-main",
            "wal_bloat",
            "/data/facts_work.sqlite",
            "critical",
            "WAL size 8.3 GB exceeds threshold 2 GB",
            NotificationKind::New,
            5,
            "2026-04-14T03:00:00Z",
        );

        let payload = build_slack_payload(&n, 35100, "https://nq.example");
        let text = payload["text"].as_str().unwrap();
        let headline = text.lines().next().unwrap();

        assert!(headline.contains("CRITICAL on labelwatch-main"), "headline: {}", headline);
        assert!(text.contains("• `wal_bloat` on `/data/facts_work.sqlite`"), "finding line: {}", text);
        assert!(text.contains("Generation #35100 · 5 consecutive (new)"), "new metadata line: {}", text);
        assert!(text.contains("_Source:_ WAL size 8.3 GB exceeds threshold 2 GB"), "evidence footer: {}", text);
    }

    #[test]
    fn discord_render_parallels_slack_shape() {
        let n = sample_notification(
            "driftwatch-main",
            "disk_pressure",
            "/data",
            "warning",
            "disk at 87% (threshold 85%)",
            NotificationKind::Escalated { from_severity: "info".into() },
            12,
            "2026-04-14T02:00:00Z",
        );

        let payload = build_discord_payload(&n, 35200, "https://nq.example");
        let content = payload["content"].as_str().unwrap();
        let headline = content.lines().next().unwrap();

        assert!(headline.contains("WARNING on driftwatch-main"), "headline: {}", headline);
        assert!(content.contains("• `disk_pressure` on `/data`"), "finding line: {}", content);
        assert!(content.contains("Escalated from info · generation #35200 · 12 consecutive"), "metadata: {}", content);
        assert!(content.contains("Since 2026-04-14 02:00 UTC"), "since line: {}", content);
        assert!(content.contains("[detail](https://nq.example"), "detail link: {}", content);
        assert!(content.contains("_Source:_ disk at 87%"), "evidence footer: {}", content);
    }

    #[test]
    fn slack_renders_regime_line_when_annotation_present() {
        let mut n = sample_notification(
            "host-1",
            "wal_bloat",
            "/db",
            "warning",
            "WAL at 8GB",
            NotificationKind::New,
            12,
            "2026-04-14T00:00:00Z",
        );
        n.regime = Some((
            crate::regime::RegimeBadge::Resolving,
            "host disk_used_pct settling after prior pressure".to_string(),
        ));

        let payload = build_slack_payload(&n, 100, "https://nq.example");
        let text = payload["text"].as_str().unwrap();
        assert!(
            text.contains("Regime: resolving — host disk_used_pct settling after prior pressure"),
            "regime line missing: {}", text
        );
    }

    #[test]
    fn slack_omits_regime_line_when_annotation_absent() {
        let n = sample_notification(
            "host-1",
            "wal_bloat",
            "/db",
            "warning",
            "WAL at 8GB",
            NotificationKind::New,
            12,
            "2026-04-14T00:00:00Z",
        );
        // regime remains None from sample_notification default.
        let payload = build_slack_payload(&n, 100, "https://nq.example");
        let text = payload["text"].as_str().unwrap();
        assert!(!text.contains("Regime:"), "must not emit regime line when absent: {}", text);
    }

    #[test]
    fn discord_renders_regime_line_via_small_text_marker() {
        let mut n = sample_notification(
            "host-1",
            "wal_bloat",
            "/db",
            "warning",
            "WAL at 8GB",
            NotificationKind::New,
            12,
            "2026-04-14T00:00:00Z",
        );
        n.regime = Some((
            crate::regime::RegimeBadge::Worsening,
            "recovery lag is pathological against its own baseline".to_string(),
        ));

        let payload = build_discord_payload(&n, 100, "https://nq.example");
        let content = payload["content"].as_str().unwrap();
        // Discord uses -# for secondary lines — confirm the regime line
        // is demoted, not elevated to a body heading.
        assert!(
            content.contains("-# Regime: worsening — recovery lag is pathological against its own baseline"),
            "discord regime line missing: {}", content
        );
    }

    #[test]
    fn webhook_payload_carries_regime_fields() {
        let mut n = sample_notification(
            "host-1",
            "wal_bloat",
            "/db",
            "warning",
            "WAL at 8GB",
            NotificationKind::New,
            12,
            "2026-04-14T00:00:00Z",
        );
        n.regime = Some((
            crate::regime::RegimeBadge::Stable,
            "entrenched finding, recovery within baseline (60 gens)".to_string(),
        ));

        let payload = build_webhook_payload(&n, 100, "https://nq.example");
        assert_eq!(payload["finding"]["regime_badge"], "stable");
        assert_eq!(
            payload["finding"]["regime_explanation"],
            "entrenched finding, recovery within baseline (60 gens)"
        );
    }

    #[test]
    fn webhook_payload_regime_fields_null_when_absent() {
        let n = sample_notification(
            "host-1",
            "wal_bloat",
            "/db",
            "warning",
            "WAL at 8GB",
            NotificationKind::New,
            12,
            "2026-04-14T00:00:00Z",
        );
        let payload = build_webhook_payload(&n, 100, "https://nq.example");
        assert!(payload["finding"]["regime_badge"].is_null());
        assert!(payload["finding"]["regime_explanation"].is_null());
    }

    #[test]
    fn format_since_renders_absolute_and_relative_for_past_timestamp() {
        // Fix "now" to make relative calculation deterministic.
        let now = time::OffsetDateTime::parse(
            "2026-04-14T06:00:00Z",
            &time::format_description::well_known::Rfc3339,
        ).unwrap();

        let rendered = format_since("2026-04-14T05:15:00Z", now);
        assert!(rendered.contains("2026-04-14 05:15 UTC"), "{}", rendered);
        assert!(rendered.contains("~45m ago"), "{}", rendered);
    }

    #[test]
    fn format_since_handles_future_timestamp_without_relative() {
        let now = time::OffsetDateTime::parse(
            "2026-04-14T06:00:00Z",
            &time::format_description::well_known::Rfc3339,
        ).unwrap();

        let rendered = format_since("2099-01-01T00:00:00Z", now);
        assert!(rendered.contains("2099-01-01 00:00 UTC"), "{}", rendered);
        assert!(!rendered.contains("ago"), "future timestamps must not claim 'ago': {}", rendered);
    }

    #[test]
    fn format_since_falls_back_on_unparseable_input() {
        let now = time::OffsetDateTime::now_utc();
        let rendered = format_since("not-a-timestamp", now);
        assert_eq!(rendered, "not-a-timestamp");
    }
}
