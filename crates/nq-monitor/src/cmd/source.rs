//! `nq-monitor source retire|unretire` — the explicit evidence-retirement verb
//! (EVIDENCE_RETIREMENT_GAP). Retirement is an operator act, never inferred from
//! silence: `retire` records a deliberately torn-down source and moves the
//! findings it backs to `retired`; `unretire` reverses the current state (to
//! `unknown`, never auto-`live`) while the `finding_transitions` audit trail
//! survives. Both are atomic and idempotent.

use anyhow::{Context, Result};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::cli::{SourceAction, SourceCmd, SourceRetireCmd, SourceUnretireCmd};

pub fn run(cmd: SourceCmd) -> Result<()> {
    match cmd.action {
        SourceAction::Retire(c) => retire(c),
        SourceAction::Unretire(c) => unretire(c),
    }
}

fn retire(cmd: SourceRetireCmd) -> Result<()> {
    let now = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .context("format now")?;
    let mut db = nq_db::open_rw(&cmd.db)
        .with_context(|| format!("open db {}", cmd.db.display()))?;
    nq_db::migrate(&mut db).context("migrate")?;

    let stats = nq_db::retire_source(
        &mut db,
        &cmd.source_id,
        &cmd.reason,
        nq_db::LOCAL_OPERATOR_ACTOR,
        &now,
    )
    .with_context(|| format!("retire source {}", cmd.source_id))?;

    if stats.newly_retired {
        println!(
            "retired source '{}' ({} finding(s) -> retired): {}",
            stats.source_id, stats.findings_transitioned, cmd.reason
        );
    } else {
        println!(
            "source '{}' was already retired ({} additional finding(s) -> retired)",
            stats.source_id, stats.findings_transitioned
        );
    }
    Ok(())
}

fn unretire(cmd: SourceUnretireCmd) -> Result<()> {
    let now = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .context("format now")?;
    let mut db = nq_db::open_rw(&cmd.db)
        .with_context(|| format!("open db {}", cmd.db.display()))?;
    nq_db::migrate(&mut db).context("migrate")?;

    let stats = nq_db::unretire_source(&mut db, &cmd.source_id, nq_db::LOCAL_OPERATOR_ACTOR, &now)
        .with_context(|| format!("unretire source {}", cmd.source_id))?;

    if stats.was_retired {
        println!(
            "unretired source '{}' ({} finding(s) retired -> unknown; audit trail preserved)",
            stats.source_id, stats.findings_transitioned
        );
    } else {
        println!("source '{}' was not retired; nothing to do", stats.source_id);
    }
    Ok(())
}
