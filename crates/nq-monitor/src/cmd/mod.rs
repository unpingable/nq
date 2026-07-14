pub mod check;
pub mod drill;
pub mod emit_escalation;
pub mod findings;
pub mod fleet;
pub mod inquire;
pub mod intent;
pub mod liveness;
pub mod maintenance;
pub mod preflight;
pub mod probe;
pub mod query;
pub mod receipt;
pub mod sentinel;
pub mod serve;
pub mod smoke;
pub mod source;
pub mod validate_witness;
pub mod verify;
pub mod witness;

use crate::cli::{Cli, Command};

pub async fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Serve(cmd) => serve::run(cmd).await,
        Command::Query(cmd) => query::run(cmd),
        Command::Inquire(cmd) => inquire::run(cmd),
        Command::Intent(cmd) => intent::run(cmd),
        Command::EmitEscalation(cmd) => emit_escalation::run(cmd),
        Command::Check(cmd) => check::run(cmd),
        Command::Sentinel(cmd) => sentinel::run(cmd).await,
        Command::Findings(cmd) => findings::run(cmd),
        Command::Liveness(cmd) => liveness::run(cmd),
        Command::Fleet(cmd) => fleet::run(cmd),
        Command::Maintenance(cmd) => maintenance::run(cmd),
        Command::Source(cmd) => source::run(cmd),
        Command::Preflight(cmd) => preflight::run(cmd),
        Command::ValidateWitness(cmd) => validate_witness::run(cmd),
        Command::Verify(cmd) => verify::run(cmd),
        Command::Witness(cmd) => witness::run(cmd),
        Command::Receipt(cmd) => receipt::run(cmd),
        Command::Smoke(cmd) => smoke::run(cmd),
        Command::Probe(cmd) => probe::run(cmd),
        Command::Drill(cmd) => drill::run(cmd),
    }
}
