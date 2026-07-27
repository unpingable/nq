use crate::cli::{ConfigAction, ConfigCmd};
use anyhow::Context;
use nq_core::Config;

pub fn run(command: ConfigCmd) -> anyhow::Result<()> {
    match command.action {
        ConfigAction::Validate(command) => {
            let text = std::fs::read_to_string(&command.config).with_context(|| {
                format!(
                    "cannot read aggregator configuration `{}`; no state was changed",
                    command.config.display()
                )
            })?;
            Config::from_json_str(&text).with_context(|| {
                format!(
                    "aggregator configuration `{}` was refused; no database was opened and no listener was started",
                    command.config.display()
                )
            })?;
            println!(
                "configuration valid: {} (aggregator; no state changed)",
                command.config.display()
            );
            Ok(())
        }
    }
}
