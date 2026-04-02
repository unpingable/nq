//! `nq check` — run all saved checks and report results.
//!
//! Runs each saved query that has check_mode != 'none', evaluates
//! the result, and prints pass/fail. Exit code 0 if all pass, 1 if any fail.

use crate::cli::CheckCmd;
use nq_db::{open_ro, query_read_only, QueryLimits};

pub fn run(cmd: CheckCmd) -> anyhow::Result<()> {
    let db = open_ro(&cmd.db)?;

    // Load checks
    let result = query_read_only(
        &db,
        "SELECT query_id, name, sql_text, check_mode, check_threshold, check_column
         FROM saved_queries
         WHERE check_mode IS NOT NULL AND check_mode != 'none'
         ORDER BY name",
        QueryLimits { max_rows: 100, max_time_ms: 2_000 },
    )?;

    if result.rows.is_empty() {
        println!("No checks defined. Save a query and promote it to a check.");
        return Ok(());
    }

    let mut failures = 0;

    for row in &result.rows {
        let name = &row[1];
        let sql = &row[2];
        let mode = &row[3];
        let threshold: Option<f64> = row.get(4).and_then(|s| s.parse().ok());
        let column: Option<&String> = row.get(5);

        // Run the check
        let check_result = query_read_only(
            &db,
            sql,
            QueryLimits { max_rows: 100, max_time_ms: 5_000 },
        );

        match check_result {
            Err(e) => {
                println!("FAIL  {}  (error: {})", name, e);
                failures += 1;
            }
            Ok(r) => {
                let row_count = r.rows.len();
                let failed = match mode.as_str() {
                    "non_empty" => row_count > 0,
                    "empty" => row_count == 0,
                    "threshold" => {
                        if let (Some(thresh), Some(col)) = (threshold, column) {
                            let col_idx: usize = col.parse().unwrap_or(0);
                            r.rows.iter().any(|row| {
                                row.get(col_idx)
                                    .and_then(|v| v.parse::<f64>().ok())
                                    .map(|v| v > thresh)
                                    .unwrap_or(false)
                            })
                        } else {
                            false
                        }
                    }
                    _ => false,
                };

                if failed {
                    println!("FAIL  {}  ({} rows, mode={})", name, row_count, mode);
                    failures += 1;
                } else {
                    println!("PASS  {}  ({} rows, mode={})", name, row_count, mode);
                }
            }
        }
    }

    println!();
    if failures > 0 {
        println!("{} check(s) failed", failures);
        std::process::exit(1);
    } else {
        println!("All checks passed");
    }

    Ok(())
}
