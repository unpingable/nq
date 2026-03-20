use crate::cli::QueryCmd;
use nq_db::{open_ro, query_read_only, QueryLimits, QueryResult};

pub fn run(cmd: QueryCmd) -> anyhow::Result<()> {
    let result = if let Some(ref remote) = cmd.remote {
        query_remote(remote, &cmd.sql, cmd.limit)?
    } else if let Some(ref db_path) = cmd.db {
        let db = open_ro(db_path)?;
        query_read_only(
            &db,
            &cmd.sql,
            QueryLimits {
                max_rows: cmd.limit,
                max_time_ms: 5_000,
            },
        )?
    } else {
        anyhow::bail!("specify --db <path> or --remote <url>");
    };

    match cmd.format.as_str() {
        "json" => print_json(&result),
        "csv" => print_csv(&result),
        _ => print_table(&result),
    }

    Ok(())
}

fn query_remote(base_url: &str, sql: &str, limit: usize) -> anyhow::Result<QueryResult> {
    let url = format!(
        "{}/api/query?sql={}&limit={}",
        base_url.trim_end_matches('/'),
        urlencoding(sql),
        limit
    );

    // Synchronous HTTP — nq query is not async
    let resp = reqwest::blocking::get(&url)?;
    let body: serde_json::Value = resp.json()?;

    if let Some(err) = body.get("error").and_then(|e| e.as_str()) {
        anyhow::bail!("remote error: {err}");
    }

    let columns: Vec<String> = body["columns"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let rows: Vec<Vec<String>> = body["rows"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|row| {
                    row.as_array().map(|cells| {
                        cells.iter().map(|c| c.as_str().unwrap_or("").to_string()).collect()
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let truncated = body["truncated"].as_bool().unwrap_or(false);

    Ok(QueryResult {
        columns,
        rows,
        truncated,
    })
}

fn urlencoding(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
    out
}

fn print_table(result: &QueryResult) {
    if result.columns.is_empty() {
        println!("(no columns)");
        return;
    }

    let widths: Vec<usize> = result
        .columns
        .iter()
        .enumerate()
        .map(|(i, col)| {
            let data_max = result
                .rows
                .iter()
                .map(|row| row.get(i).map(|s| s.len()).unwrap_or(0))
                .max()
                .unwrap_or(0);
            col.len().max(data_max).min(50)
        })
        .collect();

    // Header
    let header: Vec<String> = result
        .columns
        .iter()
        .zip(&widths)
        .map(|(col, w)| format!("{:<width$}", col, width = w))
        .collect();
    println!("{}", header.join(" | "));

    // Separator
    let sep: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
    println!("{}", sep.join("-+-"));

    // Rows
    for row in &result.rows {
        let cells: Vec<String> = row
            .iter()
            .zip(&widths)
            .map(|(cell, w)| {
                if cell.len() > *w {
                    format!("{}~", &cell[..*w - 1])
                } else {
                    format!("{:<width$}", cell, width = w)
                }
            })
            .collect();
        println!("{}", cells.join(" | "));
    }

    if result.truncated {
        println!("... (truncated at {} rows)", result.rows.len());
    }

    eprintln!("{} row(s)", result.rows.len());
}

fn print_json(result: &QueryResult) {
    let rows: Vec<serde_json::Value> = result
        .rows
        .iter()
        .map(|row| {
            let obj: serde_json::Map<String, serde_json::Value> = result
                .columns
                .iter()
                .zip(row.iter())
                .map(|(col, val)| (col.clone(), serde_json::Value::String(val.clone())))
                .collect();
            serde_json::Value::Object(obj)
        })
        .collect();

    println!("{}", serde_json::to_string_pretty(&rows).unwrap_or_default());
}

fn print_csv(result: &QueryResult) {
    println!("{}", result.columns.join(","));
    for row in &result.rows {
        let escaped: Vec<String> = row
            .iter()
            .map(|cell| {
                if cell.contains(',') || cell.contains('"') || cell.contains('\n') {
                    format!("\"{}\"", cell.replace('"', "\"\""))
                } else {
                    cell.clone()
                }
            })
            .collect();
        println!("{}", escaped.join(","));
    }
}
