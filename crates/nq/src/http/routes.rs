use axum::{
    extract::{Path, Query, State},
    response::Html,
    routing::get,
    Json, Router,
};
use nq_db::{overview, host_detail, query_read_only, QueryLimits, ReadDb};
use std::sync::Arc;
use tokio::sync::Mutex;

type Db = Arc<Mutex<ReadDb>>;

/// Escape HTML-special characters to prevent XSS and rendering issues.
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

pub fn router(db: Db) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/api/overview", get(api_overview))
        .route("/api/host/{name}", get(api_host))
        .route("/api/query", get(api_query))
        .with_state(db)
}

async fn index(State(db): State<Db>) -> Html<String> {
    let db = db.lock().await;
    let vm = overview(&db).unwrap_or_else(|_| nq_db::OverviewVm {
        generation_id: None,
        generated_at: None,
        generation_status: None,
        generation_age_s: None,
        hosts: vec![],
        services: vec![],
        sqlite_dbs: vec![],
        warnings: vec![],
    });

    Html(render_overview(&vm))
}

async fn api_overview(State(db): State<Db>) -> Json<serde_json::Value> {
    let db = db.lock().await;
    match overview(&db) {
        Ok(vm) => Json(serde_json::json!({
            "generation_id": vm.generation_id,
            "generated_at": vm.generated_at,
            "status": vm.generation_status,
            "age_s": vm.generation_age_s,
            "hosts": vm.hosts.len(),
            "services": vm.services.len(),
            "sqlite_dbs": vm.sqlite_dbs.len(),
            "warnings": vm.warnings.len(),
        })),
        Err(e) => Json(serde_json::json!({"error": e.to_string()})),
    }
}

async fn api_host(State(db): State<Db>, Path(name): Path<String>) -> Json<serde_json::Value> {
    let db = db.lock().await;
    match host_detail(&db, &name) {
        Ok(vm) => Json(serde_json::json!({
            "host": vm.host,
            "recent_runs": vm.recent_source_runs.len(),
        })),
        Err(e) => Json(serde_json::json!({"error": e.to_string()})),
    }
}

#[derive(serde::Deserialize)]
struct QueryParams {
    sql: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    500
}

async fn api_query(State(db): State<Db>, Query(params): Query<QueryParams>) -> Json<serde_json::Value> {
    let db = db.lock().await;
    match query_read_only(
        &db,
        &params.sql,
        QueryLimits {
            max_rows: params.limit.min(1000),
            max_time_ms: 2_000,
        },
    ) {
        Ok(result) => Json(serde_json::json!({
            "columns": result.columns,
            "rows": result.rows,
            "truncated": result.truncated,
        })),
        Err(e) => Json(serde_json::json!({"error": e.to_string()})),
    }
}

pub fn render_overview(vm: &nq_db::OverviewVm) -> String {
    let gen_line = match (&vm.generation_id, &vm.generation_status, &vm.generation_age_s) {
        (Some(id), Some(status), Some(age)) => {
            format!(
                "Gen #{id} &middot; {age}s ago &middot; {}",
                escape_html(status)
            )
        }
        _ => "No generations yet".to_string(),
    };

    let host_rows: String = vm
        .hosts
        .iter()
        .map(|h| {
            let stale = if h.stale { " (STALE)" } else { "" };
            format!(
                "<tr><td>{}{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape_html(&h.host),
                stale,
                h.cpu_load_1m
                    .map(|v| format!("{v:.1}"))
                    .unwrap_or_default(),
                h.mem_pressure_pct
                    .map(|v| format!("{v:.0}%"))
                    .unwrap_or_default(),
                h.disk_used_pct
                    .map(|v| format!("{v:.0}%"))
                    .unwrap_or_default(),
            )
        })
        .collect();

    let svc_rows: String = vm
        .services
        .iter()
        .map(|s| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape_html(&s.host),
                escape_html(&s.service),
                escape_html(&s.status),
                s.queue_depth
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
            )
        })
        .collect();

    let db_rows: String = vm
        .sqlite_dbs
        .iter()
        .map(|d| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape_html(&d.host),
                escape_html(&d.db_path),
                d.db_size_mb
                    .map(|v| format!("{v:.1}M"))
                    .unwrap_or_default(),
                d.wal_size_mb
                    .map(|v| format!("{v:.1}M"))
                    .unwrap_or_default(),
                d.checkpoint_lag_s
                    .map(|v| format!("{v}s"))
                    .unwrap_or_default(),
            )
        })
        .collect();

    let warning_lines: String = vm
        .warnings
        .iter()
        .map(|w| {
            format!(
                "<div class=\"warning\">[{}/{}] {}</div>",
                escape_html(&w.category),
                escape_html(&w.host),
                escape_html(&w.message)
            )
        })
        .collect();

    format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>notquery</title>
<meta http-equiv="refresh" content="30">
<style>
body {{ font-family: monospace; margin: 2em; background: #1a1a1a; color: #ccc; }}
h1 {{ color: #eee; margin: 0; }}
.gen {{ color: #888; margin-bottom: 1em; }}
table {{ border-collapse: collapse; margin-bottom: 1.5em; width: 100%; }}
th, td {{ text-align: left; padding: 4px 12px 4px 0; border-bottom: 1px solid #333; }}
th {{ color: #888; }}
h2 {{ color: #aaa; font-size: 1em; margin-top: 1.5em; }}
.warning {{ color: #e8a838; margin: 2px 0; }}
.sql-box {{ margin-top: 2em; }}
.sql-box textarea {{ width: 100%; height: 3em; background: #222; color: #eee; border: 1px solid #444; font-family: monospace; padding: 8px; }}
.sql-box button {{ background: #333; color: #eee; border: 1px solid #555; padding: 4px 16px; cursor: pointer; margin-top: 4px; }}
#sql-result {{ margin-top: 1em; white-space: pre-wrap; }}
</style>
</head>
<body>
<h1>notquery</h1>
<div class="gen">{gen_line}</div>

{warning_lines}

<h2>HOSTS</h2>
<table>
<tr><th>Host</th><th>CPU 1m</th><th>Mem%</th><th>Disk%</th></tr>
{host_rows}
</table>

<h2>SERVICES</h2>
<table>
<tr><th>Host</th><th>Service</th><th>Status</th><th>Queue</th></tr>
{svc_rows}
</table>

<h2>SQLITE DBS</h2>
<table>
<tr><th>Host</th><th>DB</th><th>Size</th><th>WAL</th><th>Ckpt Lag</th></tr>
{db_rows}
</table>

<div class="sql-box">
<form onsubmit="runQuery(event)">
<textarea id="sql" placeholder="SELECT * FROM hosts_current"></textarea>
<button type="submit">Run</button>
</form>
<div id="sql-result"></div>
</div>

<script>
async function runQuery(e) {{
  e.preventDefault();
  const sql = document.getElementById('sql').value;
  const res = await fetch('/api/query?sql=' + encodeURIComponent(sql));
  const data = await res.json();
  const el = document.getElementById('sql-result');
  if (data.error) {{
    el.textContent = 'ERROR: ' + data.error;
    return;
  }}
  if (!data.columns || data.columns.length === 0) {{
    el.textContent = '(no results)';
    return;
  }}
  let out = data.columns.join(' | ') + '\n';
  out += data.columns.map(c => '-'.repeat(c.length)).join('-+-') + '\n';
  for (const row of data.rows) {{
    out += row.join(' | ') + '\n';
  }}
  if (data.truncated) out += '... (truncated)\n';
  out += data.rows.length + ' row(s)';
  el.textContent = out;
}}
</script>
</body>
</html>"#
    )
}
