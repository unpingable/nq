//! `nq maintenance declare|list` — operator/agent CLI for the
//! MAINTENANCE_DECLARATION_GAP V1 annotation lane.
//!
//! V1 ships exactly two verbs: `declare` and `list`. No `clear`, `cancel`,
//! `extend`, or `update` — the storage is append-only. A wrong declaration
//! is corrected by waiting for `end_at` to pass (or by writing a new
//! declaration whose precedence supersedes it via the deterministic
//! resolution rule in `apply_maintenance_overlay`).
//!
//! The CLI enforces the constitutional rule: `--start` must be `>= now`.
//! Past-dated starts are rejected at parse time, NOT recorded as `late`
//! (V1 explicit non-goal — late state is documented in the canonical
//! model but is not in the V1 wire shape).

use anyhow::{anyhow, Context, Result};
use std::time::SystemTime;
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

use crate::cli::{
    MaintenanceAction, MaintenanceCmd, MaintenanceDeclareCmd, MaintenanceListCmd,
};

pub fn run(cmd: MaintenanceCmd) -> Result<()> {
    match cmd.action {
        MaintenanceAction::Declare(c) => declare(c),
        MaintenanceAction::List(c) => list(c),
    }
}

fn declare(cmd: MaintenanceDeclareCmd) -> Result<()> {
    let now = OffsetDateTime::now_utc();
    let start = parse_time(&cmd.start, now)
        .with_context(|| format!("--start {:?}", cmd.start))?;
    let end = parse_time(&cmd.end, now).with_context(|| format!("--end {:?}", cmd.end))?;

    // Constitutional invariant: declaration precedes effect. The 1-second
    // tolerance covers clock-drift on the same wall-clock minute; anything
    // older is past-dated and rejected.
    if start < now - Duration::seconds(1) {
        anyhow::bail!(
            "--start must be >= now — past-dated starts are rejected per V1 \
             invariant (declaration must precede effect; late state is V2+)"
        );
    }
    if end <= start {
        anyhow::bail!("--end must be after --start");
    }

    let declared_at = now.format(&Rfc3339)?;
    let start_str = start.format(&Rfc3339)?;
    let end_str = end.format(&Rfc3339)?;
    let id = mint_maintenance_id();

    let mut db = nq_db::open_rw(&cmd.db).context("opening NQ database")?;
    nq_db::migrate(&mut db).context("migrating database schema")?;
    db.conn().execute(
        "INSERT INTO maintenance_declarations
            (maintenance_id, declared_at, declared_by, start_at, end_at,
             host, kind, subject, reason)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            &id,
            &declared_at,
            &cmd.declared_by,
            &start_str,
            &end_str,
            &cmd.host,
            &cmd.kind,
            &cmd.subject,
            &cmd.reason,
        ],
    )?;

    println!("declared {id}");
    println!("  host:        {}", cmd.host);
    println!("  kind:        {}", cmd.kind);
    println!(
        "  subject:     {}",
        cmd.subject.as_deref().unwrap_or("(any — wildcard)")
    );
    println!("  start:       {start_str}");
    println!("  end:         {end_str}");
    if let Some(by) = &cmd.declared_by {
        println!("  declared_by: {by}");
    }
    if let Some(r) = &cmd.reason {
        println!("  reason:      {r}");
    }

    Ok(())
}

fn list(cmd: MaintenanceListCmd) -> Result<()> {
    let now = OffsetDateTime::now_utc();
    let now_str = now.format(&Rfc3339)?;

    let db = nq_db::open_ro(&cmd.db).context("opening NQ database")?;

    let sql = if cmd.active {
        "SELECT maintenance_id, declared_at, declared_by, start_at, end_at,
                host, kind, subject, reason
         FROM maintenance_declarations
         WHERE start_at <= ?1 AND end_at >= ?1
         ORDER BY end_at ASC, declared_at DESC"
    } else {
        "SELECT maintenance_id, declared_at, declared_by, start_at, end_at,
                host, kind, subject, reason
         FROM maintenance_declarations
         ORDER BY declared_at DESC"
    };
    let mut stmt = db.conn().prepare(sql)?;
    let rows: Vec<MaintenanceRow> = if cmd.active {
        stmt.query_map(rusqlite::params![&now_str], MaintenanceRow::from_row)?
            .collect::<Result<_, _>>()?
    } else {
        stmt.query_map([], MaintenanceRow::from_row)?
            .collect::<Result<_, _>>()?
    };

    if rows.is_empty() {
        println!(
            "(no maintenance declarations{})",
            if cmd.active { " currently active" } else { "" }
        );
        return Ok(());
    }

    for row in rows {
        let state = if row.end_at.as_str() < now_str.as_str() {
            "expired"
        } else if row.start_at.as_str() > now_str.as_str() {
            "future"
        } else {
            "active"
        };
        println!("{} [{state}]", row.maintenance_id);
        println!(
            "  scope:   host={} kind={} subject={}",
            row.host,
            row.kind,
            row.subject.as_deref().unwrap_or("(any)"),
        );
        println!("  window:  {} → {}", row.start_at, row.end_at);
        if let Some(by) = &row.declared_by {
            println!("  by:      {by}");
        }
        if let Some(r) = &row.reason {
            println!("  reason:  {r}");
        }
        println!();
    }
    Ok(())
}

struct MaintenanceRow {
    maintenance_id: String,
    declared_by: Option<String>,
    start_at: String,
    end_at: String,
    host: String,
    kind: String,
    subject: Option<String>,
    reason: Option<String>,
}

impl MaintenanceRow {
    fn from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            maintenance_id: r.get(0)?,
            // declared_at intentionally not displayed in list output —
            // the window times are what matter operationally.
            declared_by: r.get(2)?,
            start_at: r.get(3)?,
            end_at: r.get(4)?,
            host: r.get(5)?,
            kind: r.get(6)?,
            subject: r.get(7)?,
            reason: r.get(8)?,
        })
    }
}

fn parse_time(input: &str, now: OffsetDateTime) -> Result<OffsetDateTime> {
    if input == "now" {
        return Ok(now);
    }
    if let Some(rest) = input.strip_prefix("now+") {
        let dur = parse_duration(rest)?;
        return Ok(now + dur);
    }
    OffsetDateTime::parse(input, &Rfc3339)
        .map_err(|e| anyhow!("expected ISO-8601 / 'now' / 'now+30m', got {input:?}: {e}"))
}

fn parse_duration(s: &str) -> Result<Duration> {
    if s.is_empty() {
        anyhow::bail!("empty duration");
    }
    // Single trailing unit char: 30m, 2h, 1d, 600s.
    let bytes = s.as_bytes();
    let last = bytes[bytes.len() - 1] as char;
    let (num_str, unit) = (&s[..s.len() - 1], last);
    let n: i64 = num_str
        .parse()
        .map_err(|e| anyhow!("bad duration number {num_str:?}: {e}"))?;
    match unit {
        's' => Ok(Duration::seconds(n)),
        'm' => Ok(Duration::minutes(n)),
        'h' => Ok(Duration::hours(n)),
        'd' => Ok(Duration::days(n)),
        _ => anyhow::bail!("unknown duration unit {unit:?} (expected s/m/h/d)"),
    }
}

fn mint_maintenance_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("maint_{:032x}", nanos)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ref_now() -> OffsetDateTime {
        OffsetDateTime::parse("2026-05-08T12:00:00Z", &Rfc3339).unwrap()
    }

    #[test]
    fn parse_time_accepts_now() {
        let now = ref_now();
        let t = parse_time("now", now).unwrap();
        assert_eq!(t, now);
    }

    #[test]
    fn parse_time_accepts_relative_minutes() {
        let now = ref_now();
        let t = parse_time("now+30m", now).unwrap();
        assert_eq!(t, now + Duration::minutes(30));
    }

    #[test]
    fn parse_time_accepts_relative_hours() {
        let now = ref_now();
        let t = parse_time("now+2h", now).unwrap();
        assert_eq!(t, now + Duration::hours(2));
    }

    #[test]
    fn parse_time_accepts_relative_days() {
        let now = ref_now();
        let t = parse_time("now+1d", now).unwrap();
        assert_eq!(t, now + Duration::days(1));
    }

    #[test]
    fn parse_time_accepts_relative_seconds() {
        let now = ref_now();
        let t = parse_time("now+600s", now).unwrap();
        assert_eq!(t, now + Duration::seconds(600));
    }

    #[test]
    fn parse_time_accepts_iso_8601() {
        let now = ref_now();
        let t = parse_time("2030-01-01T00:00:00Z", now).unwrap();
        assert_eq!(
            t,
            OffsetDateTime::parse("2030-01-01T00:00:00Z", &Rfc3339).unwrap()
        );
    }

    #[test]
    fn parse_time_rejects_garbage() {
        let now = ref_now();
        assert!(parse_time("yesterday", now).is_err());
        assert!(parse_time("now+", now).is_err());
        assert!(parse_time("now+30x", now).is_err());
        assert!(parse_time("", now).is_err());
    }

    #[test]
    fn mint_maintenance_id_has_prefix() {
        let id = mint_maintenance_id();
        assert!(id.starts_with("maint_"));
        assert!(id.len() > "maint_".len());
    }

    #[test]
    fn mint_maintenance_id_is_unique_across_calls() {
        // Two consecutive nanos-based mints should differ.
        let a = mint_maintenance_id();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let b = mint_maintenance_id();
        assert_ne!(a, b);
    }
}
