#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod accounts;
mod models;
mod opencode;
mod store;
mod sync;

use serde_json::json;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Listener, Manager, WindowEvent,
};

// ──────── 配置读写 ────────

/// 包外数据目录覆盖。返回 None 表示沿用「exe 同目录」的既有行为。
#[cfg(target_os = "macos")]
pub(crate) fn app_data_override() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let dir = home.join("Library/Application Support/com.icehomura.opencode-monitor");
    let _ = std::fs::create_dir_all(&dir);
    Some(dir)
}

#[cfg(target_os = "linux")]
pub(crate) fn app_data_override() -> Option<std::path::PathBuf> {
    std::env::var_os("APPIMAGE")
        .map(std::path::PathBuf::from)
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub(crate) fn app_data_override() -> Option<std::path::PathBuf> {
    None
}

fn config_candidates() -> Vec<std::path::PathBuf> {
    let mut out: Vec<std::path::PathBuf> = Vec::new();
    if let Some(dir) = app_data_override() {
        out.push(dir.join("opencode-monitor.json"));
    }
    out.extend(
        [
            std::env::current_exe().ok().map(|d| {
                d.parent()
                    .unwrap_or(std::path::Path::new("."))
                    .join("opencode-monitor.json")
            }),
            std::env::current_dir().ok().map(|d| d.join("opencode-monitor.json")),
        ]
        .into_iter()
        .flatten(),
    );
    out
}

pub(crate) fn config_path() -> Option<std::path::PathBuf> {
    let candidates = config_candidates();
    candidates
        .iter()
        .find(|p| p.is_file())
        .cloned()
        .or_else(|| candidates.first().cloned())
}

/// 全进程唯一的配置读入口。
pub(crate) fn read_config_value() -> serde_json::Value {
    config_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({}))
}

fn read_saved_config_str(key: &str) -> String {
    read_config_value()
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

/// 控制台 API 基址（配置为空时用默认值）。
pub(crate) fn base_url() -> String {
    let base = read_config_value()
        .get("base_url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string());
    base.unwrap_or_else(|| opencode::DEFAULT_BASE_URL.to_string())
}

/// 增量同步间隔（秒）：默认 30，夹在 10~3600。
/// 有意保守——按分钟级刷新够用，避免对服务端造成不必要的压力。
pub(crate) fn incremental_secs() -> u64 {
    read_config_value()
        .get("incremental_secs")
        .and_then(|v| v.as_u64())
        .filter(|v| *v >= 10)
        .unwrap_or(30)
        .min(3600)
}

fn update_config_value(mutate: impl FnOnce(&mut serde_json::Value)) -> Result<(), String> {
    let path = config_path().ok_or("无法确定配置文件路径")?;
    let mut v = read_config_value();
    mutate(&mut v);
    let text = serde_json::to_string_pretty(&v).map_err(|e| format!("序列化配置失败：{e}"))?;
    if path.is_file() {
        let _ = std::fs::copy(&path, path.with_extension("json.bak"));
    }
    std::fs::write(&path, text).map_err(|e| format!("写入配置失败：{e}"))
}

// ──────── 命令 ────────

/// 本地设置。鉴权只走登录会话，不再有 API Key 字段。
#[tauri::command]
fn get_settings() -> serde_json::Value {
    let cfg = read_config_value();
    let base = cfg
        .get("base_url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(opencode::DEFAULT_BASE_URL);
    json!({
        "base_url": base,
        "incremental_secs": incremental_secs(),
    })
}

/// 保存基址 / 同步间隔，供设置面板自动保存。
#[tauri::command]
fn save_settings(base_url: String, incremental_secs: u64) -> Result<serde_json::Value, String> {
    let base = if base_url.trim().is_empty() {
        opencode::DEFAULT_BASE_URL.to_string()
    } else {
        base_url.trim().to_string()
    };
    update_config_value(|v| {
        v["base_url"] = json!(base);
        v["incremental_secs"] = json!(incremental_secs.clamp(10, 3600));
    })?;
    Ok(json!({ "ok": true }))
}

/// 当前额度。优先读缓存；未登录或未拉取时即时请求一次。
#[tauri::command]
async fn get_quota(account_id: Option<String>) -> serde_json::Value {
    let target = account_id.filter(|s| !s.trim().is_empty());
    let id = target.clone().unwrap_or_else(accounts::primary_id);
    if let Some(q) = sync::quota_for(&id) {
        return json!({ "available": true, "quota": q, "account_id": id });
    }
    let acc = match accounts::find(&id) {
        Some(a) => a,
        None => return json!({ "available": false, "reason": "尚未添加账号", "account_id": id }),
    };
    match sync::refresh_quota(&acc, None).await {
        Ok(q) => json!({ "available": true, "quota": q, "account_id": id }),
        Err(e) => json!({ "available": false, "reason": e, "account_id": id }),
    }
}

#[tauri::command]
fn get_sync_status() -> serde_json::Value {
    json!(sync::status())
}

/// 立即同步：逐个账号拉取逐条日志（需要先完成 WebView 授权）。
#[tauri::command]
async fn sync_request_logs_now(full: bool) -> Result<serde_json::Value, String> {
    let mut total = 0usize;
    let mut synced = 0usize;
    let mut errors: Vec<String> = Vec::new();
    for acc in accounts::list() {
        if !acc.logged_in() {
            continue;
        }
        match sync::sync_request_logs(&acc, full, None).await {
            Ok(n) => {
                total += n;
                synced += 1;
            }
            Err(e) => errors.push(format!("{}：{e}", acc.display_name())),
        }
    }
    if synced == 0 && !errors.is_empty() {
        return Err(errors.join("；"));
    }
    Ok(json!({ "ok": true, "rows": total, "accounts": synced, "errors": errors }))
}

/// 每个「用到的模型」的额度与实际用量（5h / 周 / 月）。
/// 额度与用量都只取**主账号**口径（多账号下与标题栏 / 第二行卡片保持一致）。
#[tauri::command]
fn get_models() -> serde_json::Value {
    let primary = accounts::primary_id();
    let q = sync::quota_for(&primary);
    let product = q
        .as_ref()
        .map(|q| q.product.clone())
        .unwrap_or_else(|| "go".into());
    let now = chrono::Utc::now().timestamp_millis();
    let parse = |s: &Option<String>| -> Option<i64> {
        s.as_ref()
            .and_then(|x| chrono::DateTime::parse_from_rfc3339(x).ok())
            .map(|d| d.timestamp_millis())
    };
    let start_5h = q
        .as_ref()
        .and_then(|q| parse(&q.five_hour.starts_at))
        .unwrap_or(now - 5 * 3600 * 1000);
    let start_week = q
        .as_ref()
        .and_then(|q| parse(&q.week.starts_at))
        .unwrap_or(now - 7 * 24 * 3600 * 1000);
    let start_month = q
        .as_ref()
        .and_then(|q| parse(&q.starts_at))
        .unwrap_or(now - 30 * 24 * 3600 * 1000);

    let u5 = store::model_usage_since(start_5h, &primary);
    let uw = store::model_usage_since(start_week, &primary);
    let um = store::model_usage_since(start_month, &primary);

    let mut used: Vec<String> = Vec::new();
    for list in [&u5, &uw, &um] {
        for m in list {
            if !used.iter().any(|x| x.eq_ignore_ascii_case(&m.model)) {
                used.push(m.model.clone());
            }
        }
    }

    let find = |list: &[store::ModelUsage], id: &str| -> serde_json::Value {
        match list.iter().find(|m| m.model.eq_ignore_ascii_case(id)) {
            Some(m) => json!({
                "requests": m.requests,
                "input_tokens": m.input_tokens,
                "output_tokens": m.output_tokens,
                "cache_read_tokens": m.cache_read_tokens,
                "cost_micro_cents": m.cost_micro_cents
            }),
            None => json!({"requests":0,"input_tokens":0,"output_tokens":0,"cache_read_tokens":0,"cost_micro_cents":0}),
        }
    };

    let items: Vec<serde_json::Value> = used
        .iter()
        .map(|id| {
            let limit = models::lookup(id);
            let plan = limit.map(|l| l.plan(&product));
            json!({
                "id": id,
                "name": limit.map(|l| l.name.clone()).unwrap_or_else(|| id.clone()),
                "known": limit.is_some(),
                "limit": plan.map(|p| json!({
                    "usd": p.usd,
                    "req_5h": p.req_5h,
                    "req_week": p.req_week,
                    "req_month": p.req_month,
                    "unlimited": p.unlimited()
                })),
                "tokens_per_request": limit.and_then(|l| l.tokens_per_request.clone()),
                "usage": {
                    "five_hour": find(&u5, id),
                    "week": find(&uw, id),
                    "month": find(&um, id),
                }
            })
        })
        .collect();

    json!({ "product": product, "models": items })
}

/// 逐条请求日志分页；`account_id` 为空表示所有账号。
#[tauri::command]
fn get_request_logs(
    range: String,
    account_id: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
) -> serde_json::Value {
    let now = chrono::Utc::now().timestamp_millis();
    let since = match range.as_str() {
        "1h" => Some(now - 3_600_000),
        "24h" => Some(now - 24 * 3_600_000),
        "7d" => Some(now - 7 * 24 * 3_600_000),
        "30d" => Some(now - 30 * 24 * 3_600_000),
        _ => None,
    };
    let account = account_id.filter(|s| !s.trim().is_empty());
    let (rows, total) =
        store::query_request_logs(since, account.as_deref(), page.unwrap_or(1), page_size.unwrap_or(50));
    json!({ "rows": rows, "total": total })
}

/// 实际 RPM 观测（官方未公开 RPM 上限）
#[tauri::command]
fn get_rpm(range: Option<String>) -> serde_json::Value {
    let window = match range.as_deref() {
        Some("24h") => 24 * 3_600_000,
        Some("7d") => 7 * 24 * 3_600_000,
        _ => 3_600_000,
    };
    json!(store::rpm_stats(window))
}

fn local_midnight_ms() -> i64 {
    use chrono::TimeZone;
    let today = chrono::Local::now().date_naive();
    match today.and_hms_opt(0, 0, 0) {
        Some(dt) => chrono::Local
            .from_local_datetime(&dt)
            .single()
            .map(|d| d.timestamp_millis())
            .unwrap_or(0),
        None => 0,
    }
}

/// 图表分桶大小：所有时间窗口统一按分钟聚合。
const BUCKET_MS: i64 = 60_000;
/// 图表 X 轴刻度粒度（与 `BUCKET_MS` 对应），只影响前端标签格式。
const GRANULARITY: &str = "minute";

/// 时间窗口起点（毫秒）。
/// 「当前5小时 / 本周 / 本月」的起点 = 官方重置时间往前推对应时长（即 meter.startsAt）。
fn range_since(range: &str) -> i64 {
    let now = chrono::Utc::now().timestamp_millis();
    let q = sync::quota_of(None);
    let parse = |s: &Option<String>| -> Option<i64> {
        s.as_ref()
            .and_then(|x| chrono::DateTime::parse_from_rfc3339(x).ok())
            .map(|d| d.timestamp_millis())
    };
    match range {
        "1h" => now - 3_600_000,
        "5h" => q
            .as_ref()
            .and_then(|q| parse(&q.five_hour.starts_at))
            .unwrap_or(now - 5 * 3_600_000),
        "today" => local_midnight_ms(),
        "week" => q
            .as_ref()
            .and_then(|q| parse(&q.week.starts_at))
            .unwrap_or(now - 7 * 24 * 3_600_000),
        "month" => q
            .as_ref()
            .and_then(|q| parse(&q.starts_at))
            .unwrap_or(now - 30 * 24 * 3_600_000),
        _ => 0,
    }
}

/// 仪表盘数据：本地逐条日志按桶聚合（请求 / 异常 / 输出 / 输入 / 缓存）。
/// `range = "custom"` 时用 `since_ms` / `until_ms`（毫秒，左闭右开）指定区间。
#[tauri::command]
async fn get_dashboard(
    range: String,
    since_ms: Option<i64>,
    until_ms: Option<i64>,
) -> serde_json::Value {
    let (local_rows, local_cost) = store::stats_summary();
    let custom = range == "custom";
    let since = if custom {
        since_ms.unwrap_or(0).max(0)
    } else {
        range_since(&range)
    };
    let until = if custom {
        until_ms.filter(|u| *u > since)
    } else {
        None
    };
    let pts = store::request_series(since, until, BUCKET_MS);
    let mut series: Vec<serde_json::Value> = pts
        .iter()
        .map(|p| {
            json!({
                "date": p.ts_ms,
                "requests": p.requests,
                "error_requests": p.error_requests,
                "input_tokens": p.input_tokens,
                "output_tokens": p.output_tokens,
                "cache_read_tokens": p.cache_read_tokens,
                "cost_micro_cents": p.cost_micro_cents,
                "total_tokens": p.total_tokens(),
            })
        })
        .collect();

    // 无逐条数据（未登录/未同步）时，回退到云端按天/小时汇总（仅总量）；
    // 自定义区间没有对应的云端聚合口径，不做回退。
    let mut fallback = false;
    let mut granularity = GRANULARITY;
    if series.is_empty() && !custom {
        fallback = true;
        if let Some(c) = accounts::primary().and_then(|a| a.session().ok()) {
            let (api_range, b) = match range.as_str() {
                "today" => ("24h", "hour"),
                "month" => ("30d", "day"),
                "all" => ("all", "day"),
                _ => ("7d", "day"),
            };
            // 云端桶（hour/day）不是分钟，刻度必须跟着实际数据走
            granularity = if b == "hour" { "hour" } else { "day" };
            series = c
                .cost_by_day(api_range, b)
                .await
                .unwrap_or_default()
                .iter()
                .map(|p| {
                    json!({
                        "date": p.date,
                        "requests": p.total_requests,
                        "error_requests": 0,
                        "input_tokens": 0,
                        "output_tokens": p.total_tokens,
                        "cache_read_tokens": 0,
                        "cost_micro_cents": p.total_cost_micro_cents,
                        "total_tokens": p.total_tokens,
                    })
                })
                .collect();
        }
    }

    json!({
        "configured": true,
        "range": range,
        "granularity": granularity,
        "fallback": fallback,
        "series": series,
        "summary": serde_json::Value::Null,
        "window_stats": store::window_stats(since, until),
        "minute": store::current_minute_tokens(),
        "local_rows": local_rows,
        "local_cost_micro_cents": local_cost,
    })
}

// ──────── 系统设置 ────────

#[tauri::command]
fn get_close_action() -> serde_json::Value {
    let action = read_saved_config_str("close_action");
    let action = if action.is_empty() { "ask".into() } else { action };
    json!({ "action": action })
}

#[tauri::command]
fn set_close_action(action: String) -> Result<serde_json::Value, String> {
    let valid = ["quit", "minimize", "ask"];
    if !valid.contains(&action.as_str()) {
        return Err(format!("无效值：{action}，可选 quit / minimize / ask"));
    }
    update_config_value(|v| v["close_action"] = json!(action))?;
    Ok(json!({ "action": action }))
}

#[tauri::command]
fn get_autostart(app: tauri::AppHandle) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
fn set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    if enabled {
        app.autolaunch().enable().map_err(|e| e.to_string())?;
    } else {
        app.autolaunch().disable().map_err(|e| e.to_string())?;
    }
    Ok(app.autolaunch().is_enabled().unwrap_or(false))
}

// ──────── 会话授权（WebView 登录）───────

/// 在任意 JSON 里递归找形如 `org_...` 的字符串（工作区 id）。
fn extract_org_id(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) if s.starts_with("org_") || s.starts_with("wrk_") => {
            Some(s.clone())
        }
        serde_json::Value::Array(a) => a.iter().find_map(extract_org_id),
        serde_json::Value::Object(o) => o.values().find_map(extract_org_id),
        _ => None,
    }
}

/// 在工作区 / 用户信息里找一个可读名称（邮箱 / 名称 / slug），找不到就返回 None。
fn extract_label(v: &serde_json::Value) -> Option<String> {
    extract_label_at(v, 0)
}

fn extract_label_at(v: &serde_json::Value, depth: usize) -> Option<String> {
    if depth > 2 {
        return None;
    }
    if let Some(o) = v.as_object() {
        for key in ["email", "name", "displayName", "slug"] {
            if let Some(s) = o.get(key).and_then(|x| x.as_str()) {
                let s = s.trim();
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
        }
        for val in o.values() {
            if let Some(found) = extract_label_at(val, depth + 1) {
                return Some(found);
            }
        }
    }
    None
}

/// 待处理的登录请求：(是否新增账号, 目标账号 id)。
static PENDING_LOGIN: std::sync::RwLock<Option<(bool, String)>> = std::sync::RwLock::new(None);

fn set_pending_login(add_new: bool, account_id: String) {
    *PENDING_LOGIN.write().unwrap_or_else(|e| e.into_inner()) = Some((add_new, account_id));
}

fn take_pending_login() -> (bool, String) {
    PENDING_LOGIN
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .unwrap_or((false, String::new()))
}

/// 打开（或聚焦）登录授权的 WebView 窗口。
/// `add_new` = true 时登录结果会新建一个账号；否则写入 `account_id`（空 = 主账号）。
#[tauri::command]
async fn open_login_window(
    app: tauri::AppHandle,
    add_new: Option<bool>,
    account_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let add_new = add_new.unwrap_or(false);
    let account_id = account_id.unwrap_or_default();
    // 需要换一个身份登录时先清 Cookie 罐：新增账号、指定账号未登录、或当前没有任何已登录账号
    let target_logged_in = if account_id.is_empty() {
        accounts::primary().map(|a| a.logged_in()).unwrap_or(false)
    } else {
        accounts::find(&account_id).map(|a| a.logged_in()).unwrap_or(false)
    };
    let need_clear = add_new || !target_logged_in;

    if let Some(w) = app.get_webview_window("auth") {
        if need_clear {
            clear_one(&w);
            if let Ok(url) = tauri::Url::parse("https://opencode.ai/console/") {
                let _ = w.navigate(url);
            }
        }
        set_pending_login(add_new, account_id);
        let _ = w.show();
        let _ = w.set_focus();
        return Ok(json!({ "opened": true, "existed": true, "add_new": add_new }));
    }
    let url = tauri::Url::parse("https://opencode.ai/console/")
        .map_err(|e| format!("无效登录地址：{e}"))?;
    tauri::WebviewWindowBuilder::new(
        &app,
        "auth",
        tauri::WebviewUrl::External(url),
    )
    .title("登录 OpenCode")
    .inner_size(1000.0, 780.0)
    .center()
    .build()
    .map_err(|e| format!("打开登录窗口失败：{e}"))?;
    set_pending_login(add_new, account_id);
    Ok(json!({ "opened": true, "existed": false, "add_new": add_new }))
}

/// 从登录窗口读取会话 Cookie。必须在 async 命令里调用（Windows 同步调用会死锁）。
#[tauri::command]
async fn capture_login(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let w = app
        .get_webview_window("auth")
        .ok_or("登录窗口未打开，请先点击「登录 OpenCode」")?;

    // 收集 console 与主域两份 Cookie，拼成完整 Cookie 头直接转发。
    let mut jar: Vec<(String, String)> = Vec::new();
    for u in ["https://opencode.ai/console/", "https://opencode.ai/"] {
        if let Ok(url) = tauri::Url::parse(u) {
            if let Ok(list) = w.cookies_for_url(url) {
                for c in list {
                    let name = c.name().to_string();
                    if !jar.iter().any(|(n, _)| *n == name) {
                        jar.push((name, c.value().to_string()));
                    }
                }
            }
        }
    }
    let has_session = jar
        .iter()
        .any(|(n, _)| n == "__Host-console_session" || n == "console_session");
    if !has_session {
        return Err("尚未检测到登录，请先在弹出窗口完成登录".into());
    }
    let cookie_header = jar
        .iter()
        .map(|(n, v)| format!("{n}={v}"))
        .collect::<Vec<_>>()
        .join("; ");

    // 多路尝试拿工作区 id 与可读名称（控制台所有请求都带 x-org-id，缺了可能 400）
    let probe = opencode::SessionClient::new(cookie_header.clone(), base_url(), None);
    let mut org_id = String::new();
    let mut label = String::new();
    for attempt in [
        probe.orgs_current().await,
        probe.orgs_list().await,
        probe.user_info().await,
    ] {
        if let Ok(v) = attempt {
            if org_id.is_empty() {
                if let Some(id) = extract_org_id(&v) {
                    org_id = id;
                }
            }
            if label.is_empty() {
                if let Some(t) = extract_label(&v) {
                    label = t;
                }
            }
            if !org_id.is_empty() && !label.is_empty() {
                break;
            }
        }
    }

    // 写入目标账号：add_new = 新建；否则更新指定账号（空 = 主账号）
    let (add_new, target_id) = take_pending_login();
    let target_id = resolve_login_target(add_new, &target_id, &org_id, label, cookie_header)?;

    // 隐藏而非销毁：视觉上等同关闭，但保留句柄以便退出时清 Cookie 罐
    let _ = w.hide();
    Ok(json!({ "ok": true, "account_id": target_id, "org_id": org_id, "cookies": jar.len() }))
}

/// 把刚捕获的会话写入目标账号，返回账号 id。
fn resolve_login_target(
    add_new: bool,
    target_id: &str,
    org_id: &str,
    label: String,
    cookie: String,
) -> Result<String, String> {
    if add_new {
        // 同一个工作区再登录一次就并入同一账号，避免重复条目
        let id = accounts::new_id(org_id);
        let name = match accounts::find(&id) {
            Some(a) if !a.name.trim().is_empty() => a.name.clone(),
            _ if !label.is_empty() => label,
            _ => format!("账号 {}", accounts::list().len() + 1),
        };
        accounts::upsert(accounts::Account {
            id: id.clone(),
            name,
            org_id: org_id.to_string(),
            cookie,
        })?;
        if accounts::primary_id().is_empty() {
            accounts::set_primary(&id)?;
        }
        return Ok(id);
    }

    let id = if !target_id.is_empty() {
        target_id.to_string()
    } else {
        let primary = accounts::primary_id();
        if primary.is_empty() {
            accounts::new_id(org_id)
        } else {
            primary
        }
    };
    match accounts::find(&id) {
        Some(mut acc) => {
            acc.cookie = cookie;
            if !org_id.trim().is_empty() {
                acc.org_id = org_id.to_string();
            }
            if acc.name.trim().is_empty() && !label.is_empty() {
                acc.name = label;
            }
            accounts::upsert(acc)?;
        }
        None => {
            accounts::upsert(accounts::Account {
                id: id.clone(),
                name: if label.is_empty() { "主账号".into() } else { label },
                org_id: org_id.to_string(),
                cookie,
            })?;
            accounts::set_primary(&id)?;
        }
    }
    Ok(id)
}

/// 账号登录状态。
#[tauri::command]
fn login_status(account_id: Option<String>) -> serde_json::Value {
    let acc = match account_id.as_deref().filter(|s| !s.is_empty()) {
        Some(id) => accounts::find(id),
        None => accounts::primary(),
    };
    match acc {
        Some(a) => json!({
            "logged_in": a.logged_in(),
            "org_id": a.org_id,
            "account_id": a.id,
            "name": a.display_name(),
        }),
        None => json!({ "logged_in": false, "org_id": "", "account_id": "", "name": "" }),
    }
}

/// 账号列表 + 主账号（含每个账号的本地条数与同步水位）。
#[tauri::command]
fn list_accounts() -> serde_json::Value {
    let primary = accounts::primary_id();
    let items: Vec<serde_json::Value> = accounts::list()
        .into_iter()
        .map(|a| {
            let (rows, last_log) = store::account_row_stats(&a.id);
            let last_sync = store::get_meta(&format!("last_sync_ms:{}", a.id))
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);
            json!({
                "id": a.id,
                "name": a.display_name(),
                "org_id": a.org_id,
                "logged_in": a.logged_in(),
                "is_primary": a.id == primary,
                "rows": rows,
                "last_log_ms": last_log,
                "last_sync_ms": last_sync,
                "quota": sync::quota_for(&a.id),
            })
        })
        .collect();
    json!({ "accounts": items, "primary": primary })
}

/// 设置主账号：标题栏与第二行卡片（额度 / 模型请求次数）随之切换。
#[tauri::command]
fn set_primary_account(id: String) -> Result<serde_json::Value, String> {
    accounts::set_primary(&id)?;
    Ok(json!({ "ok": true, "primary": id }))
}

#[tauri::command]
fn rename_account(id: String, name: String) -> Result<serde_json::Value, String> {
    accounts::rename(&id, &name)?;
    Ok(json!({ "ok": true }))
}

/// 删除账号：`purge` 为真时同时清掉本地已同步的数据。
#[tauri::command]
fn remove_account(id: String, purge: Option<bool>) -> Result<serde_json::Value, String> {
    let removed = if purge.unwrap_or(false) { store::purge_account(&id) } else { 0 };
    accounts::remove(&id)?;
    sync::set_quota_for(&id, None);
    Ok(json!({ "ok": true, "purged": removed }))
}

/// 退出登录：清掉该账号的 Cookie 与 WebView 会话（账号条目保留）。
#[tauri::command]
async fn logout(app: tauri::AppHandle, account_id: Option<String>) -> Result<serde_json::Value, String> {
    // 1) 清空 WebView 的 Cookie 罐（否则会话还在，再次登录会被立刻自动捕获）
    clear_webview_cookies(&app);
    // 2) 关掉保留的授权窗口
    if let Some(w) = app.get_webview_window("auth") {
        let _ = w.close();
    }
    // 3) 清凭据
    let id = match account_id.filter(|s| !s.trim().is_empty()) {
        Some(id) => id,
        None => accounts::primary_id(),
    };
    if !id.is_empty() {
        accounts::clear_cookie(&id)?;
        sync::set_quota_for(&id, None);
    }
    Ok(json!({ "ok": true, "account_id": id }))
}

/// 清空 WebView 的浏览数据（含 httpOnly 的会话 Cookie）。
/// 先逐个 delete_cookie（确定性），再清 profile；没有 auth 窗口就临时建一个隐藏窗口。
fn clear_webview_cookies(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("auth") {
        clear_one(&w);
        return;
    }
    if let Ok(url) = tauri::Url::parse("https://opencode.ai/") {
        if let Ok(tmp) = tauri::WebviewWindowBuilder::new(
            app,
            "auth-cleanup",
            tauri::WebviewUrl::External(url),
        )
        .visible(false)
        .build()
        {
            clear_one(&tmp);
            let _ = tmp.destroy();
        }
    }
}

fn clear_one(w: &tauri::WebviewWindow) {
    if let Ok(list) = w.cookies() {
        for c in list {
            let _ = w.delete_cookie(c);
        }
    }
    let _ = w.clear_all_browsing_data();
}

// ──────── 崩溃日志 ────────

fn write_crash_log(msg: &str) {
    let dir = app_data_override().or_else(|| {
        std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|p| p.to_path_buf()))
    });
    if let Some(dir) = dir {
        let path = dir.join("crash.log");
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            let _ = f.write_all(msg.as_bytes());
        }
    }
    eprintln!("{msg}");
}

fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = info.payload();
        let msg = if let Some(s) = payload.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "(no payload)".to_string()
        };
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "(unknown)".to_string());
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        write_crash_log(&format!(
            "\n=== PANIC {timestamp} ===\nMessage: {msg}\nLocation: {location}\n"
        ));
        default_hook(info);
    }));
}

fn main() {
    install_panic_hook();
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .setup(|app| {
            // macOS：启用原生标题栏装饰（红绿灯）
            #[cfg(target_os = "macos")]
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_decorations(true);
            }

            // DB 初始化在后台线程，避免大表迁移阻塞启动
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::task::spawn_blocking(|| {
                    store::init_db();
                    sync::restore_from_db();
                })
                .await
                .ok();
                // 恢复状态后再启动同步循环
                sync::spawn_loop(handle);
            });

            // 托盘
            let show = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
            let restart = MenuItem::with_id(app, "restart", "重启窗口", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &restart, &quit])?;

            TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("OpenCode Monitor")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.unminimize();
                            let _ = w.set_focus();
                        }
                    }
                    "restart" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.hide();
                            let _ = w.eval("location.reload()");
                            let handle = app.clone();
                            tauri::async_runtime::spawn(async move {
                                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                                if let Some(w) = handle.get_webview_window("main") {
                                    let _ = w.show();
                                    let _ = w.set_focus();
                                }
                            });
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Some(w) = tray.app_handle().get_webview_window("main") {
                            if w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false) {
                                let _ = w.hide();
                            } else {
                                let _ = w.show();
                                let _ = w.unminimize();
                                let _ = w.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            // 关闭行为
            let handle = app.handle().clone();
            app.listen("close-choice", move |event| {
                if let Ok(payload) = serde_json::from_str::<serde_json::Value>(event.payload()) {
                    let choice = payload.get("choice").and_then(|c| c.as_str()).unwrap_or("minimize");
                    let remember = payload.get("remember").and_then(|r| r.as_bool()).unwrap_or(false);
                    if remember {
                        let _ = update_config_value(|v| v["close_action"] = json!(choice));
                    }
                    match choice {
                        "quit" => handle.exit(0),
                        "minimize" => {
                            if let Some(w) = handle.get_webview_window("main") {
                                let _ = w.hide();
                            }
                        }
                        _ => {}
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            get_quota,
            get_sync_status,
            get_dashboard,
            get_close_action,
            set_close_action,
            get_autostart,
            set_autostart,
            open_login_window,
            capture_login,
            login_status,
            logout,
            list_accounts,
            set_primary_account,
            rename_account,
            remove_account,
            sync_request_logs_now,
            get_models,
            get_request_logs,
            get_rpm,
        ])
        .on_window_event(|window, event| {
            // 只拦截主窗口的关闭：授权等子窗口应能自行关闭，
            // 否则关子窗口会触发主窗口的「关闭程序/最小化」弹窗。
            if window.label() != "main" {
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                let action = read_saved_config_str("close_action");
                match action.as_str() {
                    "quit" => {}
                    "minimize" => {
                        let _ = window.hide();
                        api.prevent_close();
                    }
                    _ => {
                        api.prevent_close();
                        let _ = window.emit("close-requested", ());
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
