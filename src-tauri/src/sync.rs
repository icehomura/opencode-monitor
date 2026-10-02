//! 同步引擎：首次全量 + 之后按固定间隔增量拉取，
//! 同时刷新 Go 额度，并把状态通过 Tauri 事件推给前端。
//!
//! 同时尝试两个导出接口并合并：
//! - **v2**（`/v2/usage/export`，按天汇总，range ≤ 90d）—— 覆盖更久的历史。
//! - **v1**（`/v1/usage/export`，逐条日志，range ≤ 30d）—— 数据更新鲜；
//!   仅未迁移到 v2 的组织可用，否则 403。
//!
//! 合并策略：先写 v2，再用 v1 覆盖，因此最近的数据以 v1 为准。
//! 任一接口不可用都只记录备注、不视为失败；两个都失败才报错。
//! 唯一键 `(day,user_type,user_id,provider,model)` 保证重复同步是幂等的。

use crate::opencode::{Client, Quota, UsageDailyRow};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::RwLock;
use tauri::{AppHandle, Emitter};

/// 增量同步窗口
const INCREMENTAL_V2_RANGE: &str = "7d";
const INCREMENTAL_V1_RANGE: &str = "7d";
/// 全量同步窗口
const FULL_V2_RANGE: &str = "90d";
const FULL_V1_RANGE: &str = "30d";

#[derive(Debug, Clone, Serialize, Default)]
pub struct SyncStatus {
    pub configured: bool,
    pub syncing: bool,
    pub last_sync_ms: i64,
    pub last_full_sync_ms: i64,
    /// 最近一次秒级采样时间
    pub last_sample_ms: i64,
    pub last_error: Option<String>,
    pub local_rows: i64,
    pub total_cost_micro_cents: i64,
    /// 本次同步实际用到的数据源，如 "v2 + v1"
    pub source_note: String,
    /// 本次同步新增/处理的条数
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

/// 根据配置构造客户端；未配置 Key 时返回带说明的错误。
pub fn client() -> Result<Client, String> {
    let cfg = crate::read_config_value();
    let key = cfg
        .get("api_key")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if key.is_empty() {
        return Err("尚未设置 API Key".into());
    }
    let base = cfg
        .get("base_url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(crate::opencode::DEFAULT_BASE_URL)
        .to_string();
    Ok(Client::with_base(key, base))
}

/// 刷新额度并广播。
pub async fn refresh_quota(client: &Client, app: Option<&AppHandle>) -> Result<Quota, String> {
    let q = client.go_status().await?;
    set_quota(Some(q.clone()));
    if let Some(app) = app {
        emit(app, "quota-updated");
    }
    Ok(q)
}

/// 优先用登录会话查额度（权限超集），失败/未登录再回退 Service Key。
pub async fn refresh_quota_any(app: Option<&AppHandle>) -> Result<Quota, String> {
    let q = if let Ok(sc) = crate::session_client() {
        sc.go_status().await?
    } else {
        client()?.go_status().await?
    };
    set_quota(Some(q.clone()));
    if let Some(app) = app {
        emit(app, "quota-updated");
    }
    Ok(q)
}

/// 同时拉取 v2 与 v1 并合并。返回 (行, 数据源备注)。
///
/// v2 先写、v1 后写（覆盖），因为 v1 是逐条日志、对“今天”更新鲜。
/// 任何一方失败都只写进备注；两边都失败才返回 Err。
async fn fetch_merged(
    client: &Client,
    v2_range: &str,
    v1_range: &str,
) -> Result<(Vec<UsageDailyRow>, String), String> {
    let mut map: HashMap<(String, String, String, String, String), UsageDailyRow> = HashMap::new();
    let mut used: Vec<&str> = Vec::new();
    let mut v2_err: Option<String> = None;
    let mut v1_err: Option<String> = None;

    match client.export_usage_v2(v2_range).await {
        Ok(rows) => {
            used.push("v2");
            for r in rows {
                map.insert(key_of(&r), r);
            }
        }
        Err(e) => v2_err = Some(e),
    }
    match client.export_usage_v1(v1_range).await {
        Ok(rows) => {
            used.push("v1");
            for r in rows {
                map.insert(key_of(&r), r);
            }
        }
        Err(e) => v1_err = Some(e),
    }

    if used.is_empty() {
        let detail = [v2_err.as_deref(), v1_err.as_deref()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join("；");
        return Err(format!("v1/v2 导出均不可用：{detail}"));
    }

    let mut note = used.join(" + ");
    if used.len() == 1 {
        let missing = if used[0] == "v2" { &v2_err } else { &v1_err };
        if let Some(e) = missing {
            note = format!("{note}（另一接口不可用：{e}）");
        }
    }
    Ok((map.into_values().collect(), note))
}

fn key_of(r: &UsageDailyRow) -> (String, String, String, String, String) {
    (
        r.day.clone(),
        r.user_type.clone(),
        r.user_id.clone(),
        r.provider.clone(),
        r.model.clone(),
    )
}

fn commit_rows(rows: &[UsageDailyRow], full: bool, note: String) -> Result<usize, String> {
    let n = crate::store::upsert_rows(rows);
    let (count, cost) = crate::store::stats_summary();
    let now = chrono::Utc::now().timestamp_millis();
    crate::store::set_meta("last_sync_ms", &now.to_string());
    if full {
        crate::store::set_meta("last_full_sync_ms", &now.to_string());
        crate::store::set_meta("initialized", "1");
    }
    write_status(|st| {
        st.syncing = false;
        st.last_sync_ms = now;
        if full {
            st.last_full_sync_ms = now;
        }
        st.last_error = None;
        st.local_rows = count;
        st.total_cost_micro_cents = cost;
        st.source_note = note;
        st.last_added = n as i64;
    });
    Ok(n)
}

fn fail(err: String) {
    write_status(|st| {
        st.syncing = false;
        st.last_error = Some(err);
    });
}

/// 增量同步：合并最近 7 天的 v2 + v1。
pub async fn sync_incremental(client: &Client, app: Option<&AppHandle>) -> Result<usize, String> {
    let _guard = SYNC_LOCK.lock().await;
    write_status(|st| st.syncing = true);
    let result = match fetch_merged(client, INCREMENTAL_V2_RANGE, INCREMENTAL_V1_RANGE).await {
        Ok((rows, note)) => commit_rows(&rows, false, note),
        Err(e) => Err(e),
    };
    if let Err(e) = &result {
        fail(e.clone());
    }
    if let Some(app) = app {
        emit(app, "sync-status");
    }
    result
}

/// 全量同步：v2 拉 90 天 + v1 拉 30 天，合并。
pub async fn sync_full(client: &Client, app: Option<&AppHandle>) -> Result<usize, String> {
    let _guard = SYNC_LOCK.lock().await;
    write_status(|st| st.syncing = true);
    let result = match fetch_merged(client, FULL_V2_RANGE, FULL_V1_RANGE).await {
        Ok((rows, note)) => commit_rows(&rows, true, note),
        Err(e) => Err(e),
    };
    if let Err(e) = &result {
        fail(e.clone());
    }
    if let Some(app) = app {
        emit(app, "sync-status");
    }
    result
}

/// 秒级采样：用 `summary?since=<上次采样时间>` 拉增量，按毫秒时间戳落库。
/// 失败时不推进 `last_sample_iso`，下次会把这次漏掉的数据补上。
pub async fn sample_usage(client: &Client) -> Result<bool, String> {
    let now = chrono::Utc::now();
    let since = crate::store::get_meta("last_sample_iso")
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            (now - chrono::Duration::seconds(5))
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
        });
    let s = client.summary_since(&since).await?;
    let has = !s.is_empty();
    if has {
        crate::store::insert_sample(now.timestamp_millis(), &s);
    }
    crate::store::set_meta("last_sample_iso", &now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
    crate::store::set_meta("last_sample_ms", &now.timestamp_millis().to_string());
    write_status(|st| st.last_sample_ms = now.timestamp_millis());
    // 保留 7 天
    crate::store::prune_samples(now.timestamp_millis() - 7 * 24 * 3600 * 1000);
    Ok(has)
}

/// 用登录会话拉取逐条日志并落库（cursor 分页），同时聚合到按天表。
/// 这是唯一能拿到秒级/逐条数据的方式（Service Key 访问 /request-logs 恒 403）。
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
    crate::store::prune_request_logs(now - 40 * 24 * 3600 * 1000);

    let (count, cost) = crate::store::stats_summary();
    let (log_count, _, _) = crate::store::request_log_stats();
    write_status(|st| {
        st.syncing = false;
        st.last_sync_ms = now;
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
    let last_sample = crate::store::get_meta("last_sample_ms")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    let configured = client().is_ok();
    *STATUS.write().unwrap_or_else(|e| e.into_inner()) = Some(SyncStatus {
        configured,
        syncing: false,
        last_sync_ms: last,
        last_full_sync_ms: last_full,
        last_sample_ms: last_sample,
        last_error: None,
        local_rows: rows,
        total_cost_micro_cents: cost,
        source_note: String::new(),
        last_added: 0,
    });
}

/// 后台循环：按配置的间隔做增量同步 + 刷新额度。
/// 启动时若从未全量同步过，先做一次全量。
pub fn spawn_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        // 启动首同步：优先用登录会话拉逐条日志，否则回退云端汇总 + 采样。
        if let Ok(c) = client() {
            let initialized = crate::store::get_meta("initialized").as_deref() == Some("1");
            let _ = refresh_quota_any(Some(&app)).await;
            if let Ok(sc) = crate::session_client() {
                let _ = sync_request_logs(&sc, !initialized, Some(&app)).await;
            } else {
                let _ = sample_usage(&c).await;
                if initialized {
                    let _ = sync_incremental(&c, Some(&app)).await;
                } else {
                    let _ = sync_full(&c, Some(&app)).await;
                }
            }
            let _ = refresh_quota_any(Some(&app)).await;
        } else if let Ok(sc) = crate::session_client() {
            let _ = refresh_quota_any(Some(&app)).await;
            let _ = sync_request_logs(&sc, true, Some(&app)).await;
        }

        loop {
            let secs = crate::incremental_secs().max(2);
            tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
            if let Err(e) = refresh_quota_any(Some(&app)).await {
                write_status(|st| st.last_error = Some(e));
            }
            if let Ok(sc) = crate::session_client() {
                let _ = sync_request_logs(&sc, false, Some(&app)).await;
            } else if let Ok(c) = client() {
                let _ = sample_usage(&c).await;
                let _ = sync_incremental(&c, Some(&app)).await;
            }
            emit(&app, "sync-status");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 真机端到端：内存库 + 真实 API，验证「v1/v2 合并 → upsert → 统计」「刷新额度」。
    /// 需要 `OPENCODE_API_KEY=... cargo test -- --ignored live_full_sync`。
    #[tokio::test]
    #[ignore]
    async fn live_full_sync_pipeline() {
        crate::store::use_memory_db_for_test();
        let key = std::env::var("OPENCODE_API_KEY").expect("需设置 OPENCODE_API_KEY");
        let c = Client::with_base(key, crate::opencode::DEFAULT_BASE_URL);

        let n = sync_full(&c, None).await.expect("全量同步应成功");
        assert!(n > 0, "应写入至少一条记录");
        let (count, cost) = crate::store::stats_summary();
        assert!(count > 0, "本地应有用量记录");
        assert!(cost > 0, "本地应有花费");
        assert!(!status().source_note.is_empty(), "应记录数据源");

        // 再次全量：upsert 幂等，行数不应增加
        let _ = sync_full(&c, None).await.expect("重复全量应成功");
        let (count2, _) = crate::store::stats_summary();
        assert_eq!(count, count2, "重复同步不应新增记录");

        let q = refresh_quota(&c, None).await.expect("额度刷新应成功");
        assert!(q.month.limit_micro_cents > 0);
    }
}
