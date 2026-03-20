//! Hostile-input tests for the read-only query guardrails in `nq_db::query`.

use nq_db::{migrate, open_ro, open_rw, query_read_only, QueryLimits};
use std::path::Path;

/// Helper: create a migrated DB on disk, return (dir_guard, path).
/// We need a real file because open_rw and open_ro on ":memory:" would be
/// two separate databases.
fn setup_db() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let mut wdb = open_rw(&db_path).unwrap();
    migrate(&mut wdb).unwrap();
    // Drop the write connection so the RO connection can open cleanly.
    drop(wdb);
    (dir, db_path)
}

fn ro(path: &Path) -> nq_db::ReadDb {
    open_ro(path).unwrap()
}

// ── 1. Recursive CTE timeout ────────────────────────────────────────────────

#[test]
fn recursive_cte_is_interrupted() {
    let (_dir, path) = setup_db();
    let db = ro(&path);

    let limits = QueryLimits {
        max_rows: 1_000_000, // high row cap so timeout fires first
        max_time_ms: 100,    // very short timeout
    };

    // Infinite recursive CTE — without the timeout this would run forever.
    let sql = "WITH RECURSIVE r(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM r) SELECT * FROM r";
    let result = query_read_only(&db, sql, limits);
    assert!(
        result.is_err(),
        "recursive CTE should be interrupted, got Ok with {} rows",
        result.as_ref().map_or(0, |r| r.rows.len())
    );
    let err_msg = result.unwrap_err().to_string().to_ascii_lowercase();
    assert!(
        err_msg.contains("interrupt") || err_msg.contains("cancel"),
        "error should mention interruption, got: {err_msg}"
    );
}

// ── 2. Row cap ──────────────────────────────────────────────────────────────

#[test]
fn row_cap_truncates() {
    let (_dir, path) = setup_db();
    let db = ro(&path);

    let limits = QueryLimits {
        max_rows: 10,
        max_time_ms: 5_000,
    };

    // CTE that generates 1000 rows
    let sql = "WITH RECURSIVE r(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM r WHERE n < 1000) \
               SELECT n FROM r";
    let result = query_read_only(&db, sql, limits).unwrap();
    assert_eq!(result.rows.len(), 10);
    assert!(result.truncated, "should be marked truncated");
}

// ── 3. Reject INSERT ────────────────────────────────────────────────────────

#[test]
fn reject_insert() {
    let (_dir, path) = setup_db();
    let db = ro(&path);

    let sql = "INSERT INTO generations(id) VALUES('evil')";
    let err = query_read_only(&db, sql, QueryLimits::default()).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("not allowed"),
        "INSERT should be rejected, got: {msg}"
    );
}

// ── 4. Reject multi-statement ───────────────────────────────────────────────

#[test]
fn reject_multi_statement() {
    let (_dir, path) = setup_db();
    let db = ro(&path);

    let sql = "SELECT 1; DROP TABLE generations";
    let err = query_read_only(&db, sql, QueryLimits::default()).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("one statement"),
        "multi-statement should be rejected, got: {msg}"
    );
}

// ── 5. Reject ATTACH ────────────────────────────────────────────────────────

#[test]
fn reject_attach() {
    let (_dir, path) = setup_db();
    let db = ro(&path);

    let sql = "ATTACH DATABASE ':memory:' AS evil";
    let err = query_read_only(&db, sql, QueryLimits::default()).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("not allowed"),
        "ATTACH should be rejected, got: {msg}"
    );
}

// ── 6. Reject PRAGMA ────────────────────────────────────────────────────────

#[test]
fn reject_pragma() {
    let (_dir, path) = setup_db();
    let db = ro(&path);

    let sql = "PRAGMA table_info(generations)";
    let err = query_read_only(&db, sql, QueryLimits::default()).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("not allowed"),
        "PRAGMA should be rejected, got: {msg}"
    );
}

// ── 7. Empty query ──────────────────────────────────────────────────────────

#[test]
fn reject_empty_query() {
    let (_dir, path) = setup_db();
    let db = ro(&path);

    let err = query_read_only(&db, "", QueryLimits::default()).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("empty"), "empty query should be rejected, got: {msg}");
}

#[test]
fn reject_whitespace_only_query() {
    let (_dir, path) = setup_db();
    let db = ro(&path);

    let err = query_read_only(&db, "   \n\t  ", QueryLimits::default()).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("empty"), "whitespace-only should be rejected, got: {msg}");
}

// ── 8. Comment evasion ──────────────────────────────────────────────────────

#[test]
fn reject_comment_evasion_block_comment() {
    let (_dir, path) = setup_db();
    let db = ro(&path);

    // Block comment hiding an INSERT
    let sql = "/* sneaky */ INSERT INTO generations(id) VALUES('evil')";
    let err = query_read_only(&db, sql, QueryLimits::default()).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("not allowed") || msg.contains("only SELECT"),
        "comment-prefixed INSERT should be rejected, got: {msg}"
    );
}

#[test]
fn reject_comment_evasion_line_comment() {
    let (_dir, path) = setup_db();
    let db = ro(&path);

    // Line comment before INSERT
    let sql = "-- hello\nINSERT INTO generations(id) VALUES('evil')";
    let err = query_read_only(&db, sql, QueryLimits::default()).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("not allowed") || msg.contains("only SELECT"),
        "line-comment-prefixed INSERT should be rejected, got: {msg}"
    );
}

// ── 9. SELECT with subquery containing DELETE ───────────────────────────────

#[test]
fn reject_delete_in_subquery() {
    let (_dir, path) = setup_db();
    let db = ro(&path);

    let sql = "SELECT * FROM (DELETE FROM generations RETURNING *)";
    let err = query_read_only(&db, sql, QueryLimits::default()).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("not allowed"),
        "DELETE in subquery should be rejected, got: {msg}"
    );
}

// ── Bonus: verify a legit query works ───────────────────────────────────────

#[test]
fn legit_select_works() {
    let (_dir, path) = setup_db();
    let db = ro(&path);

    let result = query_read_only(&db, "SELECT 1 AS val", QueryLimits::default()).unwrap();
    assert_eq!(result.columns, vec!["val"]);
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0][0], "1");
    assert!(!result.truncated);
}

#[test]
fn trailing_semicolon_is_allowed() {
    let (_dir, path) = setup_db();
    let db = ro(&path);

    let result = query_read_only(&db, "SELECT 1;", QueryLimits::default()).unwrap();
    assert_eq!(result.rows.len(), 1);
}
