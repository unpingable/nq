pub mod check;
pub mod collect;
pub mod publish;
pub mod query;
pub mod sentinel;
pub mod serve;

use crate::cli::{Cli, Command};

pub async fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Publish(cmd) => publish::run(cmd).await,
        Command::Serve(cmd) => serve::run(cmd).await,
        Command::Query(cmd) => query::run(cmd),
        Command::Collect(cmd) => collect::run(cmd),
        Command::Check(cmd) => check::run(cmd),
        Command::Sentinel(cmd) => sentinel::run(cmd).await,
    }
}
