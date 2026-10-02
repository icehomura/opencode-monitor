//! 同步引擎：按固定间隔用登录会话拉取逐条请求日志并聚合成按天行，
//! 同时刷新 Go 额度，并把状态通过 Tauri 事件推给前端。
//!
//! 唯一数据源是控制台会话（`/request-logs`）：Service API Key 恒 403，
//! 因此程序不再支持 Key 鉴权，未登录时同步直接跳过。
//! 逐条日志的主键 `id` 保证重复同步是幂等的。

use crate::opencode::{Quota, SessionClient};
use serde::Serialize;
use std::sync::RwLock;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize, Default)]
pub struct SyncStatus {
    pub syncing: bool,
    pub last_sync_ms: i64,
    pub last_full_sync_ms: i64,
    pub last_error: Option<String>,
    pub local_rows: i64,
    pub total_cost_micro_cents: i64,
    /// 本次同步的数据源备注，如 "request-logs ×123"
    pub source_note: String,
    /// 本次同步处理的条数
    pub last_added: i64,
}

static STATUS: RwLock<Option<SyncStatus>> = RwLock::new(None);
static QUOTA: RwLock<Option<Quota>> = RwLock::new(None);
static SYNC_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn read_status() -> SyncStatus {
    STATUS
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or_default()
}

fn write_status(f: impl FnOnce(&mut SyncStatus)) {
    let mut guard = STATUS.write().unwrap_or_else(|e| e.into_inner());
    let mut st = guard.clone().unwrap_or_default();
    f(&mut st);
    *guard = Some(st);
}

pub fn status() -> SyncStatus {
    read_status()
}

pub fn quota() -> Option<Quota> {
    QUOTA.read().unwrap_or_else(|e| e.into_inner()).clone()
}

pub fn set_quota(q: Option<Quota>) {
    *QUOTA.write().unwrap_or_else(|e| e.into_inner()) = q;
}

fn emit(app: &AppHandle, event: &str) {
    let _ = app.emit(event, ());
}

/// 从配置里的会话 Cookie 构造客户端（带浏览器 UA / Referer / x-org-id）；未登录时报错。
pub fn session() -> Result<SessionClient, String> {
    let cfg = crate::read_config_value();
    let cookie = cfg
        .get("session_cookie")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if cookie.is_empty() {
        return Err("未登录 OpenCode".into());
    }
    let base = cfg
        .get("base_url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(crate::opencode::DEFAULT_BASE_URL)
        .to_string();
    let org = cfg
        .get("org_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_string());
    Ok(SessionClient::new(cookie, base, org))
}

/// 刷新额度并广播（仅登录会话；未登录直接报错）。
pub async fn refresh_quota(app: Option<&AppHandle>) -> Result<Quota, String> {
    let q = session()?.go_status().await?;
    set_quota(Some(q.clone()));
    if let Some(app) = app {
        emit(app, "quota-updated");
    }
    Ok(q)
}

/// 拉取逐条日志分页，返回 (写入行数, 最大 started_at_ms)。
async fn pull_request_log_pages(
    sc: &crate::opencode::SessionClient,
    since: i64,
    until: i64,
) -> Result<(usize, i64), String> {
    let mut cursor: Option<String> = None;
    let mut total = 0usize;
    let mut max_started = since;
    let mut pages = 0;
    loop {
        pages += 1;
        if pages > 600 {
            break;
        }
        let page = sc
            .request_logs_page(since, Some(until), cursor.as_deref(), 100)
            .await?;
        if page.items.is_empty() {
            break;
        }
        for l in &page.items {
            if l.started_at_ms > max_started {
                max_started = l.started_at_ms;
            }
        }
        total += crate::store::insert_request_logs(&page.items);
        match page.next_cursor {
            Some(c) if !c.is_empty() => cursor = Some(c),
            _ => break,
        }
    }
    Ok((total, max_started))
}

/// 用登录会话拉取逐条日志并落库（cursor 分页），同时聚合到按天表；返回处理条数。
/// 这是唯一的数据通道（Service Key 访问 /request-logs 恒 403）。
pub async fn sync_request_logs(
    sc: &crate::opencode::SessionClient,
    full: bool,
    app: Option<&AppHandle>,
) -> Result<usize, String> {
    let _guard = SYNC_LOCK.lock().await;
    write_status(|st| st.syncing = true);
    let now = chrono::Utc::now().timestamp_millis();
    let since = if full {
        now - 30 * 24 * 3600 * 1000
    } else {
        crate::store::get_meta("last_requestlog_ms")
            .and_then(|s| s.parse::<i64>().ok())
            .map(|v| (v - 60_000).max(0))
            .unwrap_or(now - 24 * 3600 * 1000)
    };

    // 400 常见于 since 超出保留期或协议细节；全量时逐步缩小窗口重试。
    let mut windows: Vec<i64> = vec![since];
    if full {
        windows.push(now - 7 * 24 * 3600 * 1000);
        windows.push(now - 24 * 3600 * 1000);
    }
    let mut total = 0usize;
    let mut max_started = since;
    let mut last_err: Option<String> = None;
    for w in windows {
        match pull_request_log_pages(sc, w, now).await {
            Ok((n, m)) => {
                total = n;
                max_started = m;
                last_err = None;
                break;
            }
            Err(e) => last_err = Some(e),
        }
    }
    if let Some(e) = last_err {
        write_status(|st| {
            st.syncing = false;
            st.last_error = Some(e.clone());
        });
        if let Some(app) = app {
            emit(app, "sync-status");
        }
        return Err(e);
    }

    // 聚合成按天行，供日志表 / 汇总使用
    let daily = crate::store::request_logs_to_daily();
    crate::store::upsert_rows(&daily);
    crate::store::set_meta("last_requestlog_ms", &(max_started + 1).to_string());
    crate::store::set_meta("last_sync_ms", &now.to_string());
    crate::store::prune_request_logs(now - 40 * 24 * 3600 * 1000);
    if full {
        crate::store::set_meta("last_full_sync_ms", &now.to_string());
        crate::store::set_meta("initialized", "1");
    }

    let (count, cost) = crate::store::stats_summary();
    let (log_count, _, _) = crate::store::request_log_stats();
    write_status(|st| {
        st.syncing = false;
        st.last_sync_ms = now;
        if full {
            st.last_full_sync_ms = now;
        }
        st.last_error = None;
        st.local_rows = count.max(log_count);
        st.total_cost_micro_cents = cost;
        st.source_note = format!("request-logs ×{total}");
        st.last_added = total as i64;
    });
    if let Some(app) = app {
        emit(app, "sync-status");
    }
    Ok(total)
}

/// 从本地 meta 恢复状态（启动时调用一次）。
pub fn restore_from_db() {
    let (rows, cost) = crate::store::stats_summary();
    let last = crate::store::get_meta("last_sync_ms")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    let last_full = crate::store::get_meta("last_full_sync_ms")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    *STATUS.write().unwrap_or_else(|e| e.into_inner()) = Some(SyncStatus {
        syncing: false,
        last_sync_ms: last,
        last_full_sync_ms: last_full,
        last_error: None,
        local_rows: rows,
        total_cost_micro_cents: cost,
        source_note: String::new(),
        last_added: 0,
    });
}

/// 后台循环：按配置的间隔刷新额度 + 用登录会话增量同步逐条日志。
/// 启动时若从未全量同步过，先做一次全量。
pub fn spawn_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let initialized = crate::store::get_meta("initialized").as_deref() == Some("1");
        let _ = refresh_quota(Some(&app)).await;
        if let Ok(sc) = session() {
            let _ = sync_request_logs(&sc, !initialized, Some(&app)).await;
            let _ = refresh_quota(Some(&app)).await;
        }

        loop {
            let secs = crate::incremental_secs().max(2);
            tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
            match refresh_quota(Some(&app)).await {
                Ok(_) => {}
                Err(e) => write_status(|st| st.last_error = Some(e)),
            }
            match session() {
                Ok(sc) => {
                    let _ = sync_request_logs(&sc, false, Some(&app)).await;
                }
                Err(e) => write_status(|st| st.last_error = Some(e)),
            }
            emit(&app, "sync-status");
        }
    });
}
