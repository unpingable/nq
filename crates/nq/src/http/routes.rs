use axum::{
    extract::{Path, Query, State},
    response::Html,
    routing::{get, post, delete},
    Json, Router,
};
use nq_db::{overview, host_detail, query_read_only, QueryLimits, ReadDb, WriteDb};
use std::sync::Arc;
use tokio::sync::Mutex;

type Db = Arc<Mutex<ReadDb>>;
type WDb = Arc<Mutex<WriteDb>>;

/// Percent-encode a string for use in URL paths.
fn urlencod(s: &str) -> String {
    s.bytes().map(|b| match b {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
            String::from(b as char)
        }
        _ => format!("%{:02X}", b),
    }).collect()
}

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
        .route("/api/findings", get(api_findings))
        .route("/api/host/{name}", get(api_host))
        .route("/api/host/{name}/history", get(api_host_history))
        .route("/api/query", get(api_query))
        .route("/finding/{kind}/{host}", get(finding_detail))
        .route("/finding/{kind}/{host}/{subject}", get(finding_detail_with_subject))
        .with_state(db)
}

#[derive(Clone)]
pub struct AppState {
    pub read_db: Db,
    pub write_db: WDb,
}

pub fn router_with_write(read_db: Db, write_db: WDb) -> Router {
    let state = AppState { read_db: read_db.clone(), write_db };

    // Saved query routes use AppState, everything else delegates to read-only router
    Router::new()
        .route("/api/saved", get(api_saved_list).post(api_saved_create))
        .route("/api/saved/{id}/run", get(api_saved_run))
        .route("/api/saved/{id}", delete(api_saved_delete))
        .route("/api/saved/{id}/check", post(api_saved_promote_check))
        .route("/api/finding/transition", post(api_finding_transition))
        .with_state(state)
        .merge(router(read_db))
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
        history_generations: 0,
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

async fn api_findings(State(db): State<Db>) -> Json<serde_json::Value> {
    let db = db.lock().await;
    match query_read_only(
        &db,
        "SELECT severity, domain, kind, host, subject, message, consecutive_gens, first_seen_at, acknowledged FROM v_warnings",
        QueryLimits { max_rows: 500, max_time_ms: 2_000 },
    ) {
        Ok(result) => Json(serde_json::json!({
            "columns": result.columns,
            "rows": result.rows,
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

async fn finding_detail(
    State(db): State<Db>,
    Path((kind, host)): Path<(String, String)>,
) -> Html<String> {
    finding_detail_inner(db, &kind, &host, "").await
}

async fn finding_detail_with_subject(
    State(db): State<Db>,
    Path((kind, host, subject)): Path<(String, String, String)>,
) -> Html<String> {
    finding_detail_inner(db, &kind, &host, &subject).await
}

async fn finding_detail_inner(db: Db, kind: &str, host: &str, subject: &str) -> Html<String> {
    let db = db.lock().await;

    // Get the finding from warning_state
    let finding = query_read_only(
        &db,
        &format!(
            "SELECT severity, domain, kind, host, subject, message,
                    first_seen_gen, first_seen_at, last_seen_gen, last_seen_at,
                    consecutive_gens, peak_value, acknowledged, notified_severity, notified_at,
                    work_state, owner, note, external_ref
             FROM warning_state
             WHERE kind = '{}' AND host = '{}' AND subject = '{}'",
            kind.replace('\'', "''"),
            host.replace('\'', "''"),
            subject.replace('\'', "''"),
        ),
        QueryLimits { max_rows: 1, max_time_ms: 2_000 },
    );

    // Get related findings on the same host
    let related = query_read_only(
        &db,
        &format!(
            "SELECT severity, domain, kind, subject, message, consecutive_gens
             FROM warning_state
             WHERE host = '{}' AND NOT (kind = '{}' AND subject = '{}')
             ORDER BY consecutive_gens DESC",
            host.replace('\'', "''"),
            kind.replace('\'', "''"),
            subject.replace('\'', "''"),
        ),
        QueryLimits { max_rows: 20, max_time_ms: 2_000 },
    );

    // Get host history for context
    let host_history = query_read_only(
        &db,
        &format!(
            "SELECT g.completed_at, h.cpu_load_1m, h.mem_pressure_pct, h.disk_used_pct
             FROM hosts_history h
             JOIN generations g ON g.generation_id = h.generation_id
             WHERE h.host = '{}'
             ORDER BY g.generation_id DESC LIMIT 30",
            host.replace('\'', "''"),
        ),
        QueryLimits { max_rows: 30, max_time_ms: 2_000 },
    );

    // Build pivot queries based on detector kind
    let pivots = build_pivots(kind, host, subject);

    Html(render_finding_detail(kind, host, subject, &finding, &related, &host_history, &pivots))
}

fn build_pivots(kind: &str, host: &str, subject: &str) -> Vec<(&'static str, String)> {
    let h = host.replace('\'', "''");
    let s = subject.replace('\'', "''");
    let mut pivots: Vec<(&str, String)> = vec![
        ("All findings on this host", format!(
            "SELECT severity, domain, kind, subject, message, consecutive_gens FROM warning_state WHERE host = '{}' ORDER BY consecutive_gens DESC", h
        )),
        ("Transition history", format!(
            "SELECT created_at, from_state, to_state, changed_by, note FROM finding_transitions WHERE host = '{}' AND kind = '{}' AND subject = '{}' ORDER BY created_at DESC LIMIT 20", h, kind.replace('\'', "''"), s
        )),
    ];

    match kind {
        "freelist_bloat" | "wal_bloat" => {
            pivots.push(("SQLite DB details", format!(
                "SELECT * FROM v_sqlite_dbs WHERE host = '{}' AND db_path = '{}'", h, s
            )));
            pivots.push(("All SQLite DBs on this host", format!(
                "SELECT db_path, db_size_mb, wal_size_mb, wal_pct, freelist_pct FROM v_sqlite_dbs WHERE host = '{}'", h
            )));
        }
        "disk_pressure" | "mem_pressure" | "resource_drift" => {
            pivots.push(("Host details", format!(
                "SELECT * FROM v_hosts WHERE host = '{}'", h
            )));
            pivots.push(("Host history (last 60)", format!(
                "SELECT g.completed_at, h.cpu_load_1m, h.mem_pressure_pct, h.disk_used_pct, h.disk_avail_mb FROM hosts_history h JOIN generations g ON g.generation_id = h.generation_id WHERE h.host = '{}' ORDER BY g.generation_id DESC LIMIT 60", h
            )));
        }
        "service_flap" | "service_status" => {
            pivots.push(("Service history", format!(
                "SELECT g.completed_at, s.service, s.status FROM services_history s JOIN generations g ON g.generation_id = s.generation_id WHERE s.host = '{}' AND s.service = '{}' ORDER BY g.generation_id DESC LIMIT 30", h, s
            )));
            pivots.push(("All services on this host", format!(
                "SELECT service, status FROM v_services WHERE host = '{}'", h
            )));
        }
        "signal_dropout" => {
            pivots.push(("Series info", format!(
                "SELECT * FROM series WHERE metric_name = '{}'", s
            )));
            pivots.push(("Source health", format!(
                "SELECT * FROM v_sources"
            )));
        }
        "stale_host" | "stale_service" => {
            pivots.push(("Source runs", format!(
                "SELECT generation_id, status, duration_ms, error_message FROM source_runs WHERE source = '{}' ORDER BY generation_id DESC LIMIT 20", h
            )));
        }
        "log_silence" | "error_shift" => {
            pivots.push(("Log observation history", format!(
                "SELECT g.completed_at, lo.source_id, lo.lines_total, lo.lines_error, lo.fetch_status FROM log_observations_history lo JOIN generations g ON g.generation_id = lo.generation_id WHERE lo.host = '{}' AND lo.source_id = '{}' ORDER BY lo.generation_id DESC LIMIT 30", h, s
            )));
            pivots.push(("Log exemplars (current)", format!(
                "SELECT source_id, examples_json FROM log_observations_current WHERE host = '{}' AND source_id = '{}'", h, s
            )));
        }
        "check_failed" | "check_error" => {
            pivots.push(("Saved query definition", format!(
                "SELECT query_id, name, sql_text, check_mode, check_threshold, check_column FROM saved_queries WHERE query_id = {}", s.trim_start_matches('#')
            )));
            pivots.push(("Run the check query", format!(
                "SELECT * FROM saved_queries WHERE query_id = {}", s.trim_start_matches('#')
            )));
        }
        _ => {}
    }

    pivots
}

fn render_finding_detail(
    kind: &str,
    host: &str,
    subject: &str,
    finding: &Result<nq_db::QueryResult, anyhow::Error>,
    related: &Result<nq_db::QueryResult, anyhow::Error>,
    host_history: &Result<nq_db::QueryResult, anyhow::Error>,
    pivots: &[(&str, String)],
) -> String {
    // Extract finding fields
    let (severity, domain, message, first_seen, consecutive, peak, notified_sev, work_state, owner, note, ext_ref) =
        if let Ok(ref r) = finding {
            if let Some(row) = r.rows.first() {
                (
                    row.get(0).map(|s| s.as_str()).unwrap_or("?"),
                    row.get(1).map(|s| s.as_str()).unwrap_or("?"),
                    row.get(5).map(|s| s.as_str()).unwrap_or("?"),
                    row.get(7).map(|s| s.as_str()).unwrap_or("?"),
                    row.get(10).map(|s| s.as_str()).unwrap_or("0"),
                    row.get(11).map(|s| s.as_str()).unwrap_or(""),
                    row.get(13).map(|s| s.as_str()).unwrap_or("none"),
                    row.get(15).map(|s| s.as_str()).unwrap_or("new"),
                    row.get(16).map(|s| s.as_str()).unwrap_or(""),
                    row.get(17).map(|s| s.as_str()).unwrap_or(""),
                    row.get(18).map(|s| s.as_str()).unwrap_or(""),
                )
            } else {
                ("?", "?", "Finding not found", "?", "0", "", "none", "new", "", "", "")
            }
        } else {
            ("?", "?", "Error loading finding", "?", "0", "", "none", "new", "", "", "")
        };

    let domain_label = match domain {
        "Δo" => "missing", "Δs" => "skewed", "Δg" => "unstable", "Δh" => "degrading", d => d,
    };

    let sev_color = match severity {
        "critical" => "#da3633", "warning" => "#d29922", _ => "#484f58",
    };

    // Related findings
    let related_rows: String = if let Ok(ref r) = related {
        r.rows.iter().map(|row| {
            format!("<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape_html(row.get(0).map(|s| s.as_str()).unwrap_or("")),
                escape_html(row.get(1).map(|s| s.as_str()).unwrap_or("")),
                escape_html(row.get(2).map(|s| s.as_str()).unwrap_or("")),
                escape_html(row.get(4).map(|s| s.as_str()).unwrap_or("")),
                escape_html(row.get(5).map(|s| s.as_str()).unwrap_or("")),
            )
        }).collect()
    } else { String::new() };

    // Host history sparkline data (disk_used_pct as simple text for now)
    let history_summary: String = if let Ok(ref r) = host_history {
        if r.rows.len() >= 2 {
            let latest = r.rows.first().and_then(|r| r.get(3)).and_then(|s| s.parse::<f64>().ok());
            let oldest = r.rows.last().and_then(|r| r.get(3)).and_then(|s| s.parse::<f64>().ok());
            match (latest, oldest) {
                (Some(l), Some(o)) => {
                    let delta = l - o;
                    let arrow = if delta > 0.5 { "trending up" } else if delta < -0.5 { "trending down" } else { "stable" };
                    format!("<p style=\"color:#8b949e;font-size:12px;\">Disk: {:.1}% now, {:.1}% 30 gens ago ({}) | Mem: {} | CPU: {}</p>",
                        l, o, arrow,
                        r.rows.first().and_then(|r| r.get(2)).unwrap_or(&String::new()),
                        r.rows.first().and_then(|r| r.get(1)).unwrap_or(&String::new()),
                    )
                }
                _ => String::new(),
            }
        } else { String::new() }
    } else { String::new() };

    // Pivot links
    let pivot_links: String = pivots.iter().map(|(label, sql)| {
        format!(
            "<a class=\"pivot\" href=\"/\" onclick=\"event.preventDefault(); document.getElementById('sql').value = '{}'; runQuery(new Event('submit')); window.scrollTo(0, document.body.scrollHeight);\">{}</a>",
            escape_html(&sql.replace('\'', "\\'")),
            escape_html(label),
        )
    }).collect::<Vec<_>>().join(" · ");

    format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>nq — {kind}/{host}</title>
<style>
* {{ box-sizing: border-box; margin: 0; padding: 0; }}
body {{ font-family: 'SF Mono', 'Cascadia Code', 'Fira Code', monospace; background: #0d1117; color: #c9d1d9; padding: 24px; }}
a {{ color: #58a6ff; text-decoration: none; }}
a:hover {{ text-decoration: underline; }}
.back {{ font-size: 13px; margin-bottom: 16px; display: block; }}
.finding-header {{ display: flex; align-items: center; gap: 12px; margin-bottom: 8px; }}
.sev-badge {{ padding: 2px 10px; border-radius: 12px; font-size: 12px; font-weight: 600; color: #fff; background: {sev_color}; }}
.domain-badge {{ color: #8b949e; font-size: 14px; }}
h1 {{ font-size: 16px; color: #f0f6fc; }}
.meta {{ color: #8b949e; font-size: 13px; margin: 8px 0 16px 0; }}
.message {{ background: #161b22; border: 1px solid #21262d; border-radius: 6px; padding: 12px 16px; font-size: 13px; margin-bottom: 16px; }}
h2 {{ font-size: 13px; text-transform: uppercase; color: #8b949e; letter-spacing: 1px; margin: 20px 0 8px 0; }}
table {{ border-collapse: collapse; width: 100%; font-size: 13px; }}
th {{ text-align: left; padding: 6px 12px 6px 0; color: #484f58; font-weight: 500; border-bottom: 1px solid #21262d; }}
td {{ padding: 5px 12px 5px 0; border-bottom: 1px solid #161b22; }}
.pivots {{ margin: 16px 0; }}
.pivot {{ display: inline-block; background: #21262d; border: 1px solid #30363d; border-radius: 6px; padding: 4px 12px; font-size: 12px; margin: 2px; }}
.pivot:hover {{ background: #30363d; text-decoration: none; }}
.sql-box {{ margin-top: 24px; padding-top: 16px; border-top: 1px solid #21262d; }}
.sql-box textarea {{ width: 100%; height: 60px; background: #0d1117; color: #c9d1d9; border: 1px solid #30363d; border-radius: 6px; font-family: inherit; font-size: 13px; padding: 8px 12px; resize: vertical; }}
.sql-box textarea:focus {{ outline: none; border-color: #58a6ff; }}
.sql-box button {{ background: #21262d; color: #c9d1d9; border: 1px solid #30363d; border-radius: 6px; padding: 6px 16px; cursor: pointer; font-family: inherit; font-size: 13px; margin-top: 6px; }}
#sql-result {{ margin-top: 12px; white-space: pre-wrap; font-size: 12px; color: #8b949e; max-height: 400px; overflow: auto; }}
</style>
</head>
<body>
<a class="back" href="/">&larr; back to overview</a>

<div class="finding-header">
    <span class="sev-badge">{severity}</span>
    <span class="domain-badge">{domain} {domain_label}</span>
    <h1>{kind}</h1>
</div>

<div class="meta">
    {host_display}{subject_display} · {consecutive} consecutive generations · since {first_seen}{peak_display}{notified_display}
</div>

<div class="message">{message}</div>

<div style="display:flex;gap:12px;align-items:center;margin:12px 0;">
    <span style="background:#21262d;border:1px solid #30363d;border-radius:6px;padding:3px 10px;font-size:12px;">{work_state}</span>
    {owner_display}
    {ext_ref_display}
</div>
{note_display}

<div style="display:flex;gap:6px;margin:8px 0;flex-wrap:wrap;" id="lifecycle-actions">
    <button onclick="transition('acknowledged')" style="background:#21262d;color:#c9d1d9;border:1px solid #30363d;border-radius:6px;padding:4px 12px;font-size:12px;cursor:pointer;">Ack</button>
    <button onclick="transition('watching')" style="background:#21262d;color:#c9d1d9;border:1px solid #30363d;border-radius:6px;padding:4px 12px;font-size:12px;cursor:pointer;">Watch</button>
    <button onclick="transition('quiesced')" style="background:#21262d;color:#c9d1d9;border:1px solid #30363d;border-radius:6px;padding:4px 12px;font-size:12px;cursor:pointer;">Quiesce</button>
    <button onclick="transition('closed')" style="background:#21262d;color:#c9d1d9;border:1px solid #30363d;border-radius:6px;padding:4px 12px;font-size:12px;cursor:pointer;">Close</button>
    <button onclick="transition('suppressed')" style="background:#21262d;color:#484f58;border:1px solid #30363d;border-radius:6px;padding:4px 12px;font-size:12px;cursor:pointer;">Suppress</button>
    <button onclick="transition('new')" style="background:#21262d;color:#484f58;border:1px solid #30363d;border-radius:6px;padding:4px 12px;font-size:12px;cursor:pointer;">Reset</button>
</div>

{history_summary}

<h2>Pivots</h2>
<div class="pivots">{pivot_links}</div>

{related_section}

<div class="sql-box">
<h2>SQL</h2>
<form onsubmit="runQuery(event)">
<textarea id="sql" placeholder="SELECT * FROM v_warnings"></textarea>
<button type="submit">Run</button>
</form>
<div id="sql-result"></div>
</div>

<script>
async function transition(toState) {{
  var note = prompt('Note (optional):') || '';
  var owner = prompt('Owner (optional):') || '';
  var res = await fetch('/api/finding/transition', {{
    method: 'POST',
    headers: {{ 'Content-Type': 'application/json' }},
    body: JSON.stringify({{
      host: '{trans_host}',
      kind: '{trans_kind}',
      subject: '{trans_subject}',
      to_state: toState,
      note: note || null,
      owner: owner || null
    }})
  }});
  var data = await res.json();
  if (data.ok) {{ location.reload(); }}
  else {{ alert('Error: ' + (data.error || 'unknown')); }}
}}

async function runQuery(e) {{
  e.preventDefault();
  var sql = document.getElementById('sql').value;
  var res = await fetch('/api/query?sql=' + encodeURIComponent(sql));
  var data = await res.json();
  var el = document.getElementById('sql-result');
  if (data.error) {{ el.textContent = 'ERROR: ' + data.error; return; }}
  if (!data.columns || data.columns.length === 0) {{ el.textContent = '(no results)'; return; }}
  var out = data.columns.join(' | ') + '\n';
  out += data.columns.map(function(c) {{ return '-'.repeat(c.length); }}).join('-+-') + '\n';
  for (var i = 0; i < data.rows.length; i++) {{ out += data.rows[i].join(' | ') + '\n'; }}
  if (data.truncated) out += '... (truncated)\n';
  out += data.rows.length + ' row(s)';
  el.textContent = out;
}}
</script>
</body>
</html>"#,
        kind = escape_html(kind),
        host = escape_html(host),
        severity = escape_html(severity),
        sev_color = sev_color,
        domain = escape_html(domain),
        domain_label = escape_html(domain_label),
        consecutive = escape_html(consecutive),
        first_seen = escape_html(first_seen),
        message = escape_html(message),
        host_display = if host.is_empty() { String::new() } else { format!("<strong>{}</strong>", escape_html(host)) },
        subject_display = if subject.is_empty() { String::new() } else { format!(" / {}", escape_html(subject)) },
        peak_display = if peak.is_empty() { String::new() } else { format!(" · peak: {}", escape_html(peak)) },
        notified_display = if notified_sev == "none" { String::new() } else { format!(" · notified at: {}", escape_html(notified_sev)) },
        work_state = escape_html(work_state),
        owner_display = if owner.is_empty() { String::new() } else { format!("<span style=\"color:#8b949e;font-size:12px;\">owner: {}</span>", escape_html(owner)) },
        note_display = if note.is_empty() { String::new() } else { format!("<div style=\"background:#161b22;border:1px solid #21262d;border-radius:6px;padding:8px 12px;font-size:12px;color:#8b949e;margin:8px 0;\">Note: {}</div>", escape_html(note)) },
        ext_ref_display = if ext_ref.is_empty() { String::new() } else { format!("<a href=\"{}\" style=\"color:#58a6ff;font-size:12px;\">{}</a>", escape_html(ext_ref), escape_html(ext_ref)) },
        trans_host = escape_html(host),
        trans_kind = escape_html(kind),
        trans_subject = escape_html(subject),
        related_section = if related_rows.is_empty() {
            String::new()
        } else {
            format!("<h2>Related findings on this host</h2><table><tr><th>Sev</th><th>Domain</th><th>Kind</th><th>Message</th><th>Gens</th></tr>{}</table>", related_rows)
        },
    )
}

async fn api_host_history(State(db): State<Db>, Path(name): Path<String>) -> Json<serde_json::Value> {
    let db = db.lock().await;
    match query_read_only(
        &db,
        &format!(
            "SELECT g.completed_at, h.cpu_load_1m, h.mem_pressure_pct, h.disk_used_pct, h.disk_avail_mb
             FROM hosts_history h
             JOIN generations g ON g.generation_id = h.generation_id
             WHERE h.host = '{}'
             ORDER BY g.generation_id DESC LIMIT 60",
            name.replace('\'', "''")
        ),
        QueryLimits { max_rows: 60, max_time_ms: 2_000 },
    ) {
        Ok(result) => Json(serde_json::json!({
            "columns": result.columns,
            "rows": result.rows,
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
                "Gen #{id} · {age}s ago · {}",
                escape_html(status),
            )
        }
        _ => "No generations yet".to_string(),
    };

    // Separate signal from meta findings
    let signal_warnings: Vec<_> = vm.warnings.iter()
        .filter(|w| w.finding_class.as_deref().unwrap_or("signal") == "signal")
        .collect();
    let meta_warnings: Vec<_> = vm.warnings.iter()
        .filter(|w| w.finding_class.as_deref().unwrap_or("signal") == "meta")
        .collect();

    // Build terse status summary (signal only)
    let summary = if vm.generation_id.is_some() {
        let hosts_up = vm.hosts.iter().filter(|h| !h.stale).count();
        let hosts_stale = vm.hosts.iter().filter(|h| h.stale).count();
        let svcs_up = vm.services.iter().filter(|s| s.status == "up").count();
        let svcs_bad = vm.services.iter().filter(|s| s.status != "up" && s.status != "unknown").count();
        let criticals = signal_warnings.iter().filter(|w| w.severity == "critical").count();
        let warnings = signal_warnings.iter().filter(|w| w.severity == "warning").count();

        let mut parts = Vec::new();

        if hosts_stale > 0 {
            parts.push(format!("{} host(s) stale", hosts_stale));
        } else {
            parts.push(format!("{} host(s) up", hosts_up));
        }

        if svcs_bad > 0 {
            parts.push(format!("{} svc(s) down", svcs_bad));
        } else {
            parts.push(format!("{} svc(s) up", svcs_up));
        }

        if criticals > 0 {
            parts.push(format!("{} critical", criticals));
        }
        if warnings > 0 {
            parts.push(format!("{} warning", warnings));
        }

        if criticals == 0 && warnings == 0 && svcs_bad == 0 && hosts_stale == 0 {
            parts.push("no active findings".to_string());
        }

        parts.join(". ") + "."
    } else {
        String::new()
    };

    // Count findings by domain for the domain navigator (signal only)
    let mut domain_counts: std::collections::BTreeMap<&str, (usize, usize, usize)> = std::collections::BTreeMap::new();
    for w in &signal_warnings {
        let domain = w.domain.as_deref().unwrap_or("?");
        let entry = domain_counts.entry(domain).or_insert((0, 0, 0));
        match w.severity.as_str() {
            "critical" => entry.0 += 1,
            "warning" => entry.1 += 1,
            _ => entry.2 += 1,
        }
    }

    let domain_labels = [
        ("Δo", "missing", "No fresh state"),
        ("Δs", "skewed", "Invalid signal"),
        ("Δg", "unstable", "Substrate pressure"),
        ("Δh", "degrading", "Adverse trend"),
    ];

    let domain_nav: String = domain_labels
        .iter()
        .map(|(code, label, desc)| {
            let (crit, warn, info) = domain_counts.get(code as &str).copied().unwrap_or((0, 0, 0));
            let total = crit + warn + info;
            let active = if total > 0 { " active" } else { "" };
            let badge = if crit > 0 {
                format!("<span class=\"badge crit\">{}</span>", crit)
            } else if warn > 0 {
                format!("<span class=\"badge warn\">{}</span>", warn)
            } else if info > 0 {
                format!("<span class=\"badge info\">{}</span>", info)
            } else {
                String::new()
            };
            // Show warmup indicator for trend detectors
            let warmup = if *code == "Δh" && vm.history_generations < 6 {
                format!("<div class=\"domain-desc\" style=\"color:#d29922;\">warming ({}/6 gens)</div>", vm.history_generations)
            } else {
                String::new()
            };
            format!(
                "<div class=\"domain-card{active}\" data-domain=\"{code}\">
                    <div class=\"domain-header\">{badge}<span class=\"domain-code\">{code}</span> <span class=\"domain-label\">{label}</span></div>
                    <div class=\"domain-desc\">{desc}</div>
                    {warmup}
                </div>",
            )
        })
        .collect();

    let findings_rows: String = signal_warnings
        .iter()
        .map(|w| {
            let sev_class = match w.severity.as_str() {
                "critical" => "sev-crit",
                "warning" => "sev-warn",
                _ => "sev-info",
            };
            let domain = w.domain.as_deref().unwrap_or("?");
            let gens = w.consecutive_gens.map(|g| format!("{g}")).unwrap_or_default();
            let subject_path = if w.subject.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
                String::new()
            } else {
                format!("/{}", urlencod(w.subject.as_deref().unwrap_or("")))
            };
            let detail_url = format!("/finding/{}/{}{}", urlencod(&w.category), urlencod(&w.host), subject_path);
            format!(
                "<tr class=\"{sev_class}\" data-domain=\"{domain}\">
                    <td class=\"sev-dot\"></td>
                    <td>{}</td>
                    <td><a href=\"{}\">{}</a></td>
                    <td>{}</td>
                    <td>{}</td>
                    <td class=\"gens\">{}</td>
                </tr>",
                escape_html(domain),
                escape_html(&detail_url),
                escape_html(&w.category),
                escape_html(&w.host),
                escape_html(&w.message),
                escape_html(&gens),
            )
        })
        .collect();

    let host_rows: String = vm
        .hosts
        .iter()
        .map(|h| {
            let stale_class = if h.stale { " stale" } else { "" };
            format!(
                "<tr class=\"{stale_class}\"><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape_html(&h.host),
                h.cpu_load_1m.map(|v| format!("{v:.1}")).unwrap_or_default(),
                h.mem_pressure_pct.map(|v| format!("{v:.0}%")).unwrap_or_default(),
                h.disk_used_pct.map(|v| format!("{v:.0}%")).unwrap_or_default(),
                h.disk_avail_mb.map(|v| format!("{v} MB")).unwrap_or_default(),
            )
        })
        .collect();

    let db_rows: String = vm
        .sqlite_dbs
        .iter()
        .map(|d| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape_html(&d.host),
                escape_html(&d.db_path),
                d.db_size_mb.map(|v| format!("{v:.1}M")).unwrap_or_default(),
                d.wal_size_mb.map(|v| format!("{v:.1}M")).unwrap_or_default(),
            )
        })
        .collect();

    let svc_rows: String = vm
        .services
        .iter()
        .map(|s| {
            let status_class = match s.status.as_str() {
                "up" => "status-up",
                "down" => "status-down",
                "degraded" => "status-degraded",
                _ => "status-unknown",
            };
            format!(
                "<tr><td>{}</td><td>{}</td><td class=\"{status_class}\">{}</td><td>{}</td></tr>",
                escape_html(&s.host),
                escape_html(&s.service),
                escape_html(&s.status),
                s.queue_depth.map(|v| v.to_string()).unwrap_or_default(),
            )
        })
        .collect();

    format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>nq</title>
<meta http-equiv="refresh" content="30">
<style>
* {{ box-sizing: border-box; margin: 0; padding: 0; }}
body {{ font-family: 'SF Mono', 'Cascadia Code', 'Fira Code', monospace; background: #0d1117; color: #c9d1d9; }}

.header {{ background: #161b22; border-bottom: 1px solid #30363d; padding: 12px 24px; display: flex; align-items: center; gap: 16px; }}
.header h1 {{ font-size: 18px; color: #f0f6fc; font-weight: 600; }}
.header .gen {{ color: #8b949e; font-size: 13px; }}

.layout {{ display: grid; grid-template-columns: 220px 1fr; min-height: calc(100vh - 48px); }}

.sidebar {{ background: #0d1117; border-right: 1px solid #21262d; padding: 16px; }}
.sidebar h2 {{ font-size: 11px; text-transform: uppercase; color: #8b949e; letter-spacing: 1px; margin-bottom: 12px; }}

.domain-card {{ padding: 10px 12px; border-radius: 6px; margin-bottom: 8px; cursor: pointer; border: 1px solid transparent; }}
.domain-card:hover {{ background: #161b22; }}
.domain-card.active {{ background: #161b22; border-color: #30363d; }}
.domain-header {{ display: flex; align-items: center; gap: 6px; font-size: 13px; }}
.domain-code {{ color: #8b949e; }}
.domain-label {{ color: #c9d1d9; font-weight: 500; }}
.domain-desc {{ font-size: 11px; color: #484f58; margin-top: 2px; }}
.badge {{ font-size: 11px; padding: 1px 6px; border-radius: 10px; font-weight: 600; }}
.badge.crit {{ background: #da3633; color: #fff; }}
.badge.warn {{ background: #d29922; color: #fff; }}
.badge.info {{ background: #388bfd33; color: #58a6ff; }}

.main {{ padding: 20px 24px; overflow-x: auto; }}
.main h2 {{ font-size: 13px; text-transform: uppercase; color: #8b949e; letter-spacing: 1px; margin: 20px 0 8px 0; }}
.main h2:first-child {{ margin-top: 0; }}

table {{ border-collapse: collapse; width: 100%; font-size: 13px; }}
th {{ text-align: left; padding: 6px 12px 6px 0; color: #484f58; font-weight: 500; border-bottom: 1px solid #21262d; }}
td {{ padding: 5px 12px 5px 0; border-bottom: 1px solid #161b22; }}
tr.stale td {{ opacity: 0.5; }}

.sev-dot {{ width: 8px; padding-right: 4px !important; }}
tr.sev-crit .sev-dot {{ color: #da3633; }}
tr.sev-crit .sev-dot::after {{ content: '●'; }}
tr.sev-warn .sev-dot {{ color: #d29922; }}
tr.sev-warn .sev-dot::after {{ content: '●'; }}
tr.sev-info .sev-dot {{ color: #484f58; }}
tr.sev-info .sev-dot::after {{ content: '●'; }}
.gens {{ color: #484f58; font-size: 11px; }}

.status-up {{ color: #3fb950; }}
.status-down {{ color: #da3633; font-weight: 600; }}
.status-degraded {{ color: #d29922; }}
.status-unknown {{ color: #484f58; }}

.sql-box {{ margin-top: 24px; padding-top: 16px; border-top: 1px solid #21262d; }}
.sql-box textarea {{ width: 100%; height: 60px; background: #0d1117; color: #c9d1d9; border: 1px solid #30363d; border-radius: 6px; font-family: inherit; font-size: 13px; padding: 8px 12px; resize: vertical; }}
.sql-box textarea:focus {{ outline: none; border-color: #58a6ff; }}
.sql-box button {{ background: #21262d; color: #c9d1d9; border: 1px solid #30363d; border-radius: 6px; padding: 6px 16px; cursor: pointer; font-family: inherit; font-size: 13px; margin-top: 6px; }}
.sql-box button:hover {{ background: #30363d; }}
#sql-result {{ margin-top: 12px; white-space: pre-wrap; font-size: 12px; color: #8b949e; max-height: 400px; overflow: auto; }}
</style>
</head>
<body>

<div class="header">
    <h1>nq</h1>
    <span class="gen">{gen_line}</span>
    <span class="gen" style="margin-left:auto;">{summary}</span>
</div>

<div class="layout">
<div class="sidebar">
    <h2>Failure Domains</h2>
    {domain_nav}
</div>

<div class="main">

<h2>Findings ({signal_count})</h2>
<table id="findings-table">
<tr><th></th><th>Domain</th><th>Kind</th><th>Host</th><th>Message</th><th>Gens</th></tr>
{findings_rows}
</table>
{no_findings}
{meta_section}

<h2>Hosts</h2>
<table>
<tr><th>Host</th><th>CPU 1m</th><th>Mem%</th><th>Disk%</th><th>Free</th></tr>
{host_rows}
</table>

<h2>Services</h2>
<table>
<tr><th>Host</th><th>Service</th><th>Status</th><th>Queue</th></tr>
{svc_rows}
</table>

<h2>Log Sources</h2>
<div id="log-sources"><em style="color:#484f58;font-size:13px;">Loading...</em></div>

<h2>SQLite DBs</h2>
<table>
<tr><th>Host</th><th>DB</th><th>Size</th><th>WAL</th></tr>
{db_rows}
</table>

<h2>Saved Queries</h2>
<div id="saved-queries"><em style="color:#484f58;font-size:13px;">Loading...</em></div>

<div class="sql-box">
<h2>SQL</h2>
<form onsubmit="runQuery(event)">
<textarea id="sql" placeholder="SELECT * FROM v_metrics WHERE metric_name LIKE 'node_load%'"></textarea>
<div style="display:flex;gap:8px;margin-top:6px;">
<button type="submit">Run</button>
<button type="button" onclick="saveQuery()">Save</button>
</div>
</form>
<div id="sql-result"></div>
</div>

</div>
</div>

<script>
// Domain filter
document.querySelectorAll('.domain-card').forEach(card => {{
  card.addEventListener('click', () => {{
    const domain = card.dataset.domain;
    const wasActive = card.classList.contains('selected');
    document.querySelectorAll('.domain-card').forEach(c => c.classList.remove('selected'));
    if (!wasActive) {{
      card.classList.add('selected');
      card.style.borderColor = '#58a6ff';
    }} else {{
      card.style.borderColor = '';
    }}
    document.querySelectorAll('#findings-table tr[data-domain]').forEach(row => {{
      if (wasActive || row.dataset.domain === domain) {{
        row.style.display = '';
      }} else {{
        row.style.display = 'none';
      }}
    }});
  }});
}});

async function runQuery(e) {{
  e.preventDefault();
  const sql = document.getElementById('sql').value;
  const res = await fetch('/api/query?sql=' + encodeURIComponent(sql));
  const data = await res.json();
  renderResult(data);
}}

function renderResult(data) {{
  const el = document.getElementById('sql-result');
  if (data.error) {{ el.textContent = 'ERROR: ' + data.error; return; }}
  if (!data.columns || data.columns.length === 0) {{ el.textContent = '(no results)'; return; }}
  let out = data.columns.join(' | ') + '\n';
  out += data.columns.map(c => '-'.repeat(c.length)).join('-+-') + '\n';
  for (const row of data.rows) {{ out += row.join(' | ') + '\n'; }}
  if (data.truncated) out += '... (truncated)\n';
  out += data.rows.length + ' row(s)';
  el.textContent = out;
}}

async function loadSaved() {{
  var el = document.getElementById('saved-queries');
  try {{
    var res = await fetch('/api/saved');
    var data = await res.json();
    if (!data.rows || data.rows.length === 0) {{
      el.innerHTML = '<span style="color:#484f58;font-size:13px;">No saved queries yet. Write SQL below and click Save.</span>';
      return;
    }}
    var html = '';
    for (var i = 0; i < data.rows.length; i++) {{
      var r = data.rows[i];
      var id = r[0], name = r[1], desc = r[3], pinned = r[5] == "1";
      html += "<div style='display:flex;align-items:center;gap:8px;margin:4px 0;'>";
      if (pinned) html += "<span style='color:#d29922;'>*</span>";
      html += "<a href='#' onclick='event.preventDefault(); runSaved(" + id + ");' style='color:#58a6ff;font-size:13px;'>" + name + "</a>";
      var checkMode = r[4];
      if (checkMode && checkMode != "none") html += "<span style='color:#3fb950;font-size:10px;margin-left:4px;border:1px solid #3fb950;border-radius:4px;padding:0 4px;'>check:" + checkMode + "</span>";
      if (desc) html += "<span style='color:#484f58;font-size:11px;'> - " + desc + "</span>";
      html += " <a href='#' onclick='event.preventDefault(); deleteSaved(" + id + ");' style='color:#484f58;font-size:11px;'>[x]</a>";
      html += "</div>";
    }}
    el.innerHTML = html;
  }} catch(e) {{ el.textContent = 'Error loading saved queries'; }}
}}

async function runSaved(id) {{
  const res = await fetch('/api/saved/' + id + '/run');
  const data = await res.json();
  if (data.sql) document.getElementById('sql').value = data.sql;
  renderResult(data);
  window.scrollTo(0, document.body.scrollHeight);
}}

async function saveQuery() {{
  const sql = document.getElementById('sql').value;
  if (!sql.trim()) return;
  const name = prompt('Name for this query:');
  if (!name) return;
  const desc = prompt('Description (optional):') || '';
  await fetch('/api/saved', {{
    method: 'POST',
    headers: {{ 'Content-Type': 'application/json' }},
    body: JSON.stringify({{ name, sql_text: sql, description: desc || null }})
  }});
  loadSaved();
}}

async function deleteSaved(id) {{
  if (!confirm('Delete this saved query?')) return;
  await fetch('/api/saved/' + id, {{ method: 'DELETE' }});
  loadSaved();
}}

async function loadLogs() {{
  var el = document.getElementById('log-sources');
  try {{
    var res = await fetch('/api/query?sql=' + encodeURIComponent("SELECT source_id, fetch_status, lines_total, lines_error, CASE WHEN lines_total > 0 THEN ROUND(CAST(lines_error AS REAL) * 100.0 / lines_total, 1) ELSE 0 END AS error_pct, last_log_ts FROM log_observations_current ORDER BY source_id"));
    var data = await res.json();
    if (!data.rows || data.rows.length === 0) {{
      el.innerHTML = '<span style="color:#484f58;font-size:13px;">No log sources configured.</span>';
      return;
    }}
    var html = '<table><tr><th>Source</th><th>Status</th><th>Lines</th><th>Errors</th><th>Err%</th><th>Last Log</th></tr>';
    for (var i = 0; i < data.rows.length; i++) {{
      var r = data.rows[i];
      var statusColor = r[1] === "ok" ? "rgb(63,185,80)" : r[1] === "source_quiet" ? "rgb(210,153,34)" : "rgb(218,54,51)";
      html += "<tr><td>" + r[0] + "</td><td style='color:" + statusColor + ";'>" + r[1] + "</td><td>" + r[2] + "</td><td>" + r[3] + "</td><td>" + r[4] + "%</td><td style='color:#484f58;'>" + (r[5] || "-") + "</td></tr>";
    }}
    html += '</table>';
    el.innerHTML = html;
  }} catch(e) {{ el.textContent = 'Error loading log sources'; }}
}}

loadLogs();
loadSaved();
</script>
</body>
</html>"#,
        signal_count = signal_warnings.len(),
        no_findings = if signal_warnings.is_empty() { "<p style=\"color:#484f58;font-size:13px;\">No active findings.</p>" } else { "" },
        meta_section = if meta_warnings.is_empty() { String::new() } else {
            let meta_rows: String = meta_warnings.iter().map(|w| {
                format!("<tr style=\"color:#484f58;\"><td>{}</td><td>{}</td><td>{}</td></tr>",
                    escape_html(&w.category),
                    escape_html(&w.message),
                    w.consecutive_gens.map(|g| g.to_string()).unwrap_or_default(),
                )
            }).collect();
            format!("<details style=\"margin:12px 0;\"><summary style=\"color:#484f58;font-size:12px;cursor:pointer;\">Observatory health ({} meta)</summary><table style=\"font-size:12px;\"><tr><th>Kind</th><th>Message</th><th>Gens</th></tr>{}</table></details>", meta_warnings.len(), meta_rows)
        },
    )
}

// --- Saved queries API ---

async fn api_saved_list(State(state): State<AppState>) -> Json<serde_json::Value> {
    let db = state.read_db.lock().await;
    match query_read_only(
        &db,
        "SELECT query_id, name, sql_text, description, check_mode, pinned, created_at FROM saved_queries ORDER BY pinned DESC, name",
        QueryLimits { max_rows: 100, max_time_ms: 2_000 },
    ) {
        Ok(result) => Json(serde_json::json!({
            "columns": result.columns,
            "rows": result.rows,
        })),
        Err(e) => Json(serde_json::json!({"error": e.to_string()})),
    }
}

#[derive(serde::Deserialize)]
struct SavedQueryCreate {
    name: String,
    sql_text: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    pinned: bool,
}

async fn api_saved_create(
    State(state): State<AppState>,
    Json(body): Json<SavedQueryCreate>,
) -> Json<serde_json::Value> {
    let db = state.write_db.lock().await;
    let now = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .expect("timestamp");

    match db.conn().execute(
        "INSERT INTO saved_queries (name, sql_text, description, check_mode, pinned, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'none', ?4, ?5, ?5)",
        rusqlite::params![&body.name, &body.sql_text, &body.description, body.pinned as i64, &now],
    ) {
        Ok(_) => Json(serde_json::json!({"ok": true, "name": body.name})),
        Err(e) => Json(serde_json::json!({"error": e.to_string()})),
    }
}

async fn api_saved_run(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let db = state.read_db.lock().await;

    // Look up the saved query
    let sql: String = match db.conn().query_row(
        "SELECT sql_text FROM saved_queries WHERE query_id = ?1",
        [id],
        |row| row.get(0),
    ) {
        Ok(s) => s,
        Err(_) => return Json(serde_json::json!({"error": "saved query not found"})),
    };

    match query_read_only(&db, &sql, QueryLimits { max_rows: 500, max_time_ms: 5_000 }) {
        Ok(result) => Json(serde_json::json!({
            "columns": result.columns,
            "rows": result.rows,
            "truncated": result.truncated,
            "sql": sql,
        })),
        Err(e) => Json(serde_json::json!({"error": e.to_string(), "sql": sql})),
    }
}

#[derive(serde::Deserialize)]
struct PromoteCheckBody {
    check_mode: String,
    #[serde(default)]
    check_threshold: Option<f64>,
    #[serde(default)]
    check_column: Option<String>,
}

async fn api_saved_promote_check(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<PromoteCheckBody>,
) -> Json<serde_json::Value> {
    let db = state.write_db.lock().await;
    let now = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .expect("timestamp");

    match db.conn().execute(
        "UPDATE saved_queries SET check_mode = ?1, check_threshold = ?2, check_column = ?3, updated_at = ?4
         WHERE query_id = ?5",
        rusqlite::params![&body.check_mode, body.check_threshold, &body.check_column, &now, id],
    ) {
        Ok(0) => Json(serde_json::json!({"error": "not found"})),
        Ok(_) => Json(serde_json::json!({"ok": true, "check_mode": body.check_mode})),
        Err(e) => Json(serde_json::json!({"error": e.to_string()})),
    }
}

#[derive(serde::Deserialize)]
struct FindingTransition {
    host: String,
    kind: String,
    #[serde(default)]
    subject: String,
    to_state: String,
    #[serde(default)]
    changed_by: Option<String>,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    external_ref: Option<String>,
    #[serde(default)]
    suppressed_by: Option<String>,
    /// TTL in hours for ack/quiesce/suppress. After expiry, reverts to 'new'.
    #[serde(default)]
    expires_in_hours: Option<i64>,
}

async fn api_finding_transition(
    State(state): State<AppState>,
    Json(body): Json<FindingTransition>,
) -> Json<serde_json::Value> {
    // Validate state
    let valid_states = ["new", "acknowledged", "watching", "quiesced", "closed", "suppressed"];
    if !valid_states.contains(&body.to_state.as_str()) {
        return Json(serde_json::json!({"error": format!("invalid state: {}. valid: {:?}", body.to_state, valid_states)}));
    }

    let db = state.write_db.lock().await;
    let now = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .expect("timestamp");

    // Get current state
    let current_state: Option<String> = db.conn().query_row(
        "SELECT work_state FROM warning_state WHERE host = ?1 AND kind = ?2 AND subject = ?3",
        rusqlite::params![&body.host, &body.kind, &body.subject],
        |row| row.get(0),
    ).ok();

    if current_state.is_none() {
        return Json(serde_json::json!({"error": "finding not found"}));
    }

    // Update work state
    let mut updates = vec!["work_state = ?1", "work_state_at = ?2"];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
        Box::new(body.to_state.clone()),
        Box::new(now.clone()),
    ];

    if let Some(ref owner) = body.owner {
        updates.push("owner = ?");
        params.push(Box::new(owner.clone()));
    }
    if let Some(ref note) = body.note {
        updates.push("note = ?");
        params.push(Box::new(note.clone()));
    }
    if let Some(ref ext_ref) = body.external_ref {
        updates.push("external_ref = ?");
        params.push(Box::new(ext_ref.clone()));
    }

    // Build the update — simpler approach with direct params
    // Compute expiry for ack/quiesce/suppress
    let expires_at: Option<String> = body.expires_in_hours.map(|h| {
        let expiry = time::OffsetDateTime::now_utc() + time::Duration::hours(h);
        expiry.format(&time::format_description::well_known::Rfc3339).expect("timestamp")
    });

    let result = db.conn().execute(
        "UPDATE warning_state SET work_state = ?1, work_state_at = ?2, owner = COALESCE(?3, owner), note = COALESCE(?4, note), external_ref = COALESCE(?5, external_ref), suppressed_by = ?6, ack_expires_at = ?7 WHERE host = ?8 AND kind = ?9 AND subject = ?10",
        rusqlite::params![
            &body.to_state, &now,
            &body.owner, &body.note, &body.external_ref,
            &body.suppressed_by, &expires_at,
            &body.host, &body.kind, &body.subject,
        ],
    );

    if let Err(e) = result {
        return Json(serde_json::json!({"error": e.to_string()}));
    }

    // Record transition
    let _ = db.conn().execute(
        "INSERT INTO finding_transitions (host, kind, subject, from_state, to_state, changed_by, note, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            &body.host, &body.kind, &body.subject,
            &current_state, &body.to_state,
            &body.changed_by, &body.note, &now,
        ],
    );

    Json(serde_json::json!({
        "ok": true,
        "from_state": current_state,
        "to_state": body.to_state,
    }))
}

async fn api_saved_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let db = state.write_db.lock().await;
    match db.conn().execute("DELETE FROM saved_queries WHERE query_id = ?1", [id]) {
        Ok(0) => Json(serde_json::json!({"error": "not found"})),
        Ok(_) => Json(serde_json::json!({"ok": true})),
        Err(e) => Json(serde_json::json!({"error": e.to_string()})),
    }
}
