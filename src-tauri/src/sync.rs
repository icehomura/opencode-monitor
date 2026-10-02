//! 同步引擎：按固定间隔用各账号的登录会话拉取逐条请求日志并聚合成按天行，
//! 同时刷新各账号的 Go 额度，并把状态通过 Tauri 事件推给前端。
//!
//! 唯一数据源是控制台会话（`/request-logs`）：Service API Key 恒 403，
//! 因此程序不再支持 Key 鉴权，未登录的账号直接跳过。
//! 数据按 `account_id` 隔离，逐条日志主键 `(account_id, id)` 保证重复同步幂等。

use crate::opencode::Quota;
use serde::Serialize;
use std::collections::HashMap;
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
/// 每个账号的额度缓存（key = account_id）。
static QUOTAS: RwLock<Option<HashMap<String, Quota>>> = RwLock::new(None);
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

/// 某个账号的额度缓存。
pub fn quota_for(account_id: &str) -> Option<Quota> {
    QUOTAS
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|m| m.get(account_id))
        .cloned()
}

pub fn set_quota_for(account_id: &str, q: Option<Quota>) {
    let mut guard = QUOTAS.write().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    match q {
        Some(q) => {
            map.insert(account_id.to_string(), q);
        }
        None => {
            map.remove(account_id);
        }
    }
}

/// 某个账号的额度；`account_id` 为空时用主账号。
pub fn quota_of(account_id: Option<&str>) -> Option<Quota> {
    let id = match account_id {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => crate::accounts::primary_id(),
    };
    quota_for(&id)
}

fn emit(app: &AppHandle, event: &str) {
    let _ = app.emit(event, ());
}

/// 账号维度的 meta key，如 `last_requestlog_ms:org_xxx`。
fn meta_key(base: &str, account_id: &str) -> String {
    format!("{base}:{account_id}")
}

fn account_meta(base: &str, account_id: &str) -> Option<String> {
    crate::store::get_meta(&meta_key(base, account_id))
}

/// 该账号是否做过首同步。
fn initialized(account_id: &str) -> bool {
    account_meta("initialized", account_id).as_deref() == Some("1")
}

/// 刷新某个账号的额度并广播。
pub async fn refresh_quota(
    account: &crate::accounts::Account,
    app: Option<&AppHandle>,
) -> Result<Quota, String> {
    let q = account.session()?.go_status().await?;
    set_quota_for(&account.id, Some(q.clone()));
    if let Some(app) = app {
        emit(app, "quota-updated");
    }
    Ok(q)
}

/// 拉取逐条日志分页，返回 (写入行数, 最大 started_at_ms)。
async fn pull_request_log_pages(
    sc: &crate::opencode::SessionClient,
    account_id: &str,
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
        total += crate::store::insert_request_logs(&page.items, account_id);
        match page.next_cursor {
            Some(c) if !c.is_empty() => cursor = Some(c),
            _ => break,
        }
    }
    Ok((total, max_started))
}

/// 用某个账号的登录会话拉取逐条日志并落库（cursor 分页），同时聚合到按天表；
/// 返回处理条数。这是唯一的数据通道（Service Key 访问 /request-logs 恒 403）。
pub async fn sync_request_logs(
    account: &crate::accounts::Account,
    full: bool,
    app: Option<&AppHandle>,
) -> Result<usize, String> {
    let sc = account.session()?;
    let _guard = SYNC_LOCK.lock().await;
    write_status(|st| st.syncing = true);
    let now = chrono::Utc::now().timestamp_millis();
    let since = if full {
        now - 30 * 24 * 3600 * 1000
    } else {
        account_meta("last_requestlog_ms", &account.id)
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
        match pull_request_log_pages(&sc, &account.id, w, now).await {
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
            st.last_error = Some(format!("{}：{e}", account.display_name()));
        });
        if let Some(app) = app {
            emit(app, "sync-status");
        }
        return Err(e);
    }

    // 聚合成按天行，供日志表 / 汇总使用
    let daily = crate::store::request_logs_to_daily();
    crate::store::upsert_rows(&daily);
    crate::store::set_meta(&meta_key("last_requestlog_ms", &account.id), &(max_started + 1).to_string());
    crate::store::set_meta(&meta_key("last_sync_ms", &account.id), &now.to_string());
    if full {
        crate::store::set_meta(&meta_key("last_full_sync_ms", &account.id), &now.to_string());
        crate::store::set_meta(&meta_key("initialized", &account.id), "1");
        write_status(|st| st.last_full_sync_ms = now);
    }
    // 全局水位（界面展示用）取各账号里最新的
    let newest = crate::store::get_meta("last_sync_ms")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    if now > newest {
        crate::store::set_meta("last_sync_ms", &now.to_string());
    }
    crate::store::prune_request_logs(now - 40 * 24 * 3600 * 1000);

    let (count, cost) = crate::store::stats_summary();
    let (log_count, _, _) = crate::store::request_log_stats();
    write_status(|st| {
        st.syncing = false;
        st.last_sync_ms = now.max(st.last_sync_ms);
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

/// 后台循环：按配置的间隔逐个账号刷新额度 + 增量同步逐条日志。
/// 启动时每个从未全量同步过的账号先做一次全量。
pub fn spawn_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        for acc in crate::accounts::list() {
            if !acc.logged_in() {
                continue;
            }
            let _ = sync_request_logs(&acc, !initialized(&acc.id), Some(&app)).await;
            let _ = refresh_quota(&acc, Some(&app)).await;
        }

        loop {
            let secs = crate::incremental_secs().max(2);
            tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
            for acc in crate::accounts::list() {
                if !acc.logged_in() {
                    continue;
                }
                match refresh_quota(&acc, Some(&app)).await {
                    Ok(_) => {}
                    Err(e) => write_status(|st| {
                        st.last_error = Some(format!("{}：{e}", acc.display_name()))
                    }),
                }
                let _ = sync_request_logs(&acc, false, Some(&app)).await;
            }
            emit(&app, "sync-status");
        }
    });
}
