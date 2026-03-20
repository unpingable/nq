use crate::cli::CollectCmd;
use crate::collect;
use nq_core::PublisherConfig;

pub fn run(cmd: CollectCmd) -> anyhow::Result<()> {
    let config_text = std::fs::read_to_string(&cmd.config)?;
    let config: PublisherConfig = serde_json::from_str(&config_text)?;
    let state = collect::collect_state(&config);
    let json = serde_json::to_string_pretty(&state)?;
    println!("{json}");
    Ok(())
}
