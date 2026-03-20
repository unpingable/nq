//! Read-only SQL query execution with strict guardrails.
//!
//! Rules:
//! - Single statement only
//! - SELECT/WITH only
//! - Hard wall-clock timeout
//! - Row cap
//! - No ATTACH, PRAGMA, temp schema

use crate::ReadDb;

#[derive(Debug, Clone, Copy)]
pub struct QueryLimits {
    pub max_rows: usize,
    pub max_time_ms: u64,
}

impl Default for QueryLimits {
    fn default() -> Self {
        Self {
            max_rows: 500,
            max_time_ms: 2_000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub truncated: bool,
}

pub fn query_read_only(
    db: &ReadDb,
    sql: &str,
    limits: QueryLimits,
) -> anyhow::Result<QueryResult> {
    let normalized = sql.trim();

    if normalized.is_empty() {
        anyhow::bail!("empty query");
    }

    // Single statement only
    if normalized.contains(';') {
        let without_trailing = normalized.trim_end_matches(';').trim();
        if without_trailing.contains(';') {
            anyhow::bail!("only one statement allowed");
        }
    }

    let lower = normalized.to_ascii_lowercase();

    // Reject dangerous keywords
    for banned in &["attach", "detach", "pragma", "create", "drop", "alter", "insert", "update", "delete", "replace"] {
        // Check for keyword at word boundary
        if lower_contains_keyword(&lower, banned) {
            anyhow::bail!("statement type '{banned}' not allowed; only SELECT/WITH queries");
        }
    }

    if !(lower.starts_with("select") || lower.starts_with("with")) {
        anyhow::bail!("only SELECT/WITH queries allowed");
    }

    // Set up progress handler for timeout
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(limits.max_time_ms);
    db.conn.progress_handler(1000, Some(move || {
        std::time::Instant::now() > deadline
    }));

    let result = execute_query(&db.conn, normalized, limits.max_rows);

    // Remove progress handler
    db.conn.progress_handler(1000, None::<fn() -> bool>);

    result
}

fn execute_query(
    conn: &rusqlite::Connection,
    sql: &str,
    max_rows: usize,
) -> anyhow::Result<QueryResult> {
    let mut stmt = conn.prepare(sql)?;

    let columns: Vec<String> = stmt
        .column_names()
        .into_iter()
        .map(String::from)
        .collect();

    let col_count = columns.len();
    let mut rows = Vec::new();
    let mut truncated = false;

    let mut result_rows = stmt.query([])?;
    while let Some(row) = result_rows.next()? {
        if rows.len() >= max_rows {
            truncated = true;
            break;
        }
        let mut cells = Vec::with_capacity(col_count);
        for i in 0..col_count {
            let val: rusqlite::types::Value = row.get(i)?;
            cells.push(format_value(&val));
        }
        rows.push(cells);
    }

    Ok(QueryResult {
        columns,
        rows,
        truncated,
    })
}

fn format_value(val: &rusqlite::types::Value) -> String {
    match val {
        rusqlite::types::Value::Null => "NULL".to_string(),
        rusqlite::types::Value::Integer(i) => i.to_string(),
        rusqlite::types::Value::Real(f) => format!("{f}"),
        rusqlite::types::Value::Text(s) => s.clone(),
        rusqlite::types::Value::Blob(b) => format!("<blob {} bytes>", b.len()),
    }
}

fn lower_contains_keyword(lower: &str, keyword: &str) -> bool {
    // Check if keyword appears as a standalone word (not inside an identifier)
    for (i, _) in lower.match_indices(keyword) {
        let before_ok = i == 0 || !lower.as_bytes()[i - 1].is_ascii_alphanumeric() && lower.as_bytes()[i - 1] != b'_';
        let after_pos = i + keyword.len();
        let after_ok = after_pos >= lower.len() || !lower.as_bytes()[after_pos].is_ascii_alphanumeric() && lower.as_bytes()[after_pos] != b'_';
        if before_ok && after_ok {
            return true;
        }
    }
    false
}
