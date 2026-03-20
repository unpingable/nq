use crate::cli::ServeCmd;
use crate::http;
use crate::pull;
use nq_core::Config;
use nq_db::{migrate, open_ro, open_rw, publish_batch};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};

pub async fn run(cmd: ServeCmd) -> anyhow::Result<()> {
    let config_text = std::fs::read_to_string(&cmd.config)?;
    let config: Config = serde_json::from_str(&config_text)?;

    let db_path = std::path::PathBuf::from(&config.db_path);

    // Open writer and migrate
    let mut write_db = open_rw(&db_path)?;
    migrate(&mut write_db)?;

    let write_db = Arc::new(Mutex::new(write_db));
    let config = Arc::new(config);

    // Start pull loop
    let pull_db = write_db.clone();
    let pull_config = config.clone();
    tokio::spawn(async move {
        let interval = std::time::Duration::from_secs(pull_config.interval_s);
        let mut cycle: u64 = 0;
        loop {
            cycle += 1;
            match pull::pull_all(&pull_config).await {
                Ok(batch) => {
                    let mut db = pull_db.lock().await;
                    match publish_batch(&mut db, &batch) {
                        Ok(result) => {
                            info!(
                                generation = result.generation_id,
                                ok = result.sources_ok,
                                failed = result.sources_failed,
                                "published generation"
                            );
                        }
                        Err(e) => {
                            error!(err = %e, "publish failed");
                        }
                    }

                    // Retention pruning
                    if cycle % pull_config.retention.prune_every_n_cycles == 0 {
                        match nq_db::prune(&mut db, pull_config.retention.max_generations) {
                            Ok(stats) if stats.generations_pruned > 0 => {
                                info!(pruned = stats.generations_pruned, "retention prune");
                            }
                            Err(e) => {
                                error!(err = %e, "retention prune failed");
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    error!(err = %e, "pull cycle failed");
                }
            }
            tokio::time::sleep(interval).await;
        }
    });

    // Start HTTP server
    let read_db = open_ro(&db_path)?;
    let bind = "127.0.0.1:9848";
    info!(bind, "web UI starting");
    http::serve(read_db, bind).await?;
    Ok(())
}
