//! `nq witness` — producers for caller-supplied `nq.witness.v1` packets.
//!
//! Each subcommand observes one source (git working tree, pytest run,
//! ...) and emits a witness packet to stdout. Witnesses report typed
//! observations + `coverage_limits`; they do not name claims.

pub mod diff_scope;
pub mod git_status;
pub mod pytest;

use crate::cli::{WitnessAction, WitnessCmd};

pub fn run(cmd: WitnessCmd) -> anyhow::Result<()> {
    match cmd.action {
        WitnessAction::GitStatus(c) => git_status::run(c),
        WitnessAction::Pytest(c) => pytest::run(c),
        WitnessAction::DiffScope(c) => diff_scope::run(c),
    }
}

pub(crate) fn now_rfc3339() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}
