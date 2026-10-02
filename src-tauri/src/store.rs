//! 本地 SQLite 存储：把会话拉到的逐条请求日志同步到本地。
//!
//! 多账号：所有数据行都带 `account_id`，不同账号的数据互不覆盖——
//! - `usage_daily` 主键 `(account_id, day, user_type, user_id, provider, model)`
//! - `request_log` 主键 `(account_id, id)`
//!
//! `meta` 表存同步水位；与账号相关的键带后缀（如 `last_requestlog_ms:<account_id>`）。

use crate::opencode::UsageDailyRow;
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::Mutex;

const DDL_USAGE_DAILY: &str = "CREATE TABLE IF NOT EXISTS usage_daily (
    account_id TEXT NOT NULL DEFAULT '',
    day TEXT NOT NULL,
    user_type TEXT NOT NULL,
    user_id TEXT NOT NULL,
    user_name TEXT NOT NULL DEFAULT '',
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    requests INTEGER NOT NULL DEFAULT 0,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    cache_read_tokens INTEGER NOT NULL DEFAULT 0,
    cache_write_5m_tokens INTEGER NOT NULL DEFAULT 0,
    cache_write_1h_tokens INTEGER NOT NULL DEFAULT 0,
    cost_micro_cents INTEGER NOT NULL DEFAULT 0,
    synced_at INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (account_id, day, user_type, user_id, provider, model)
);";

const DDL_META: &str = "CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);";

const DDL_REQUEST_LOG: &str = "CREATE TABLE IF NOT EXISTS request_log (
    account_id TEXT NOT NULL DEFAULT '',
    id TEXT NOT NULL,
    started_at_ms INTEGER NOT NULL,
    finished_at_ms INTEGER NOT NULL DEFAULT 0,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    provider TEXT NOT NULL DEFAULT '',
    model TEXT NOT NULL DEFAULT '',
    user_id TEXT NOT NULL DEFAULT '',
    user_type TEXT NOT NULL DEFAULT '',
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    reasoning_tokens INTEGER NOT NULL DEFAULT 0,
    cache_read_tokens INTEGER NOT NULL DEFAULT 0,
    cache_write_tokens INTEGER NOT NULL DEFAULT 0,
    cache_write_1h_tokens INTEGER NOT NULL DEFAULT 0,
    cost_micro_cents INTEGER NOT NULL DEFAULT 0,
    status_code INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (account_id, id)
);";

static DB: Mutex<Option<Connection>> = Mutex::new(None);

fn db_path() -> std::path::PathBuf {
    if let Some(dir) = crate::app_data_override() {
        return dir.join("opencode-monitor-data.db");
    }
    std::env::current_exe()
        .ok()
        .and_then(|d| d.parent().map(|p| p.join("opencode-monitor-data.db")))
        .unwrap_or_else(|| std::path::PathBuf::from("opencode-monitor-data.db"))
}

fn create_schema(conn: &Connection) {
    conn.execute_batch(DDL_USAGE_DAILY).expect("创建 usage_daily 失败");
    conn.execute_batch(DDL_META).expect("创建 meta 失败");
    conn.execute_batch(DDL_REQUEST_LOG).expect("创建 request_log 失败");
}

/// 本地库只是云端日志的缓存：schema 版本对不上就整表重建，由下次同步重新拉取，
/// 因此这里不做任何数据迁移。
const SCHEMA_VERSION: i64 = 2;

fn stored_schema_version(conn: &Connection) -> i64 {
    conn.query_row(
        "SELECT value FROM meta WHERE key = 'schema_version'",
        [],
        |r| r.get::<_, String>(0),
    )
    .ok()
    .and_then(|s| s.parse::<i64>().ok())
    .unwrap_or(0)
}

fn reset_cache(conn: &Connection) {
    conn.execute_batch(
        "DROP TABLE IF EXISTS request_log;
         DROP TABLE IF EXISTS usage_daily;
         DROP TABLE IF EXISTS meta;",
    )
    .ok();
}

/// 启动时初始化；schema 版本不一致就把缓存表清掉重建。
pub fn init_db() {
    let path = db_path();
    match Connection::open(&path) {
        Ok(conn) => {
            conn.execute_batch("PRAGMA journal_mode=WAL;").ok();
            conn.execute_batch("PRAGMA busy_timeout=3000;").ok();
            if stored_schema_version(&conn) != SCHEMA_VERSION {
                reset_cache(&conn);
            }
            create_schema(&conn);
            conn.execute(
                "INSERT INTO meta(key, value) VALUES('schema_version', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![SCHEMA_VERSION.to_string()],
            )
            .ok();
            conn.execute_batch(
                "CREATE INDEX IF NOT EXISTS idx_usage_day ON usage_daily(day);
                 CREATE INDEX IF NOT EXISTS idx_usage_user ON usage_daily(user_id);
                 CREATE INDEX IF NOT EXISTS idx_usage_model ON usage_daily(model);
                 CREATE INDEX IF NOT EXISTS idx_request_log_started ON request_log(started_at_ms);",
            )
            .ok();
            *DB.lock().unwrap_or_else(|e| e.into_inner()) = Some(conn);
            println!("[store] SQLite 已就绪：{}", path.display());
        }
        Err(e) => eprintln!("[store] SQLite 打开失败：{e}，日志同步不可用"),
    }
}

#[cfg(test)]
pub(crate) fn use_memory_db_for_test() {
    let conn = Connection::open_in_memory().unwrap();
    create_schema(&conn);
    *DB.lock().unwrap_or_else(|e| e.into_inner()) = Some(conn);
}

fn with_conn<T>(f: impl FnOnce(&Connection) -> T) -> Option<T> {
    let guard = DB.lock().unwrap_or_else(|e| e.into_inner());
    guard.as_ref().map(f)
}

/// upsert 一批云端记录，返回真正写入/更新的行数。
pub fn upsert_rows(rows: &[UsageDailyRow]) -> usize {
    let now = chrono::Utc::now().timestamp_millis();
    let mut written = 0usize;
    let res = with_conn(|conn| {
        let tx = conn.unchecked_transaction().ok();
        for r in rows {
            let n = conn.execute(
                "INSERT INTO usage_daily
                   (account_id, day, user_type, user_id, user_name, provider, model,
                    requests, input_tokens, output_tokens, cache_read_tokens,
                    cache_write_5m_tokens, cache_write_1h_tokens, cost_micro_cents, synced_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)
                 ON CONFLICT(account_id, day, user_type, user_id, provider, model) DO UPDATE SET
                   user_name=excluded.user_name,
                   requests=excluded.requests,
                   input_tokens=excluded.input_tokens,
                   output_tokens=excluded.output_tokens,
                   cache_read_tokens=excluded.cache_read_tokens,
                   cache_write_5m_tokens=excluded.cache_write_5m_tokens,
                   cache_write_1h_tokens=excluded.cache_write_1h_tokens,
                   cost_micro_cents=excluded.cost_micro_cents,
                   synced_at=excluded.synced_at",
                params![
                    r.account_id, r.day, r.user_type, r.user_id, r.user_name, r.provider, r.model,
                    r.requests, r.input_tokens, r.output_tokens, r.cache_read_tokens,
                    r.cache_write_5m_tokens, r.cache_write_1h_tokens, r.cost_micro_cents, now
                ],
            );
            if n.is_ok() {
                written += 1;
            }
        }
        if let Some(tx) = tx {
            let _ = tx.commit();
        }
    });
    if res.is_none() {
        return 0;
    }
    written
}

/// 本地总量（用于同步状态展示）。
pub fn stats_summary() -> (i64, i64) {
    with_conn(|conn| {
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM usage_daily", [], |r| r.get(0))
            .unwrap_or(0);
        let cost: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(cost_micro_cents),0) FROM usage_daily",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        (count, cost)
    })
    .unwrap_or((0, 0))
}

pub fn set_meta(key: &str, value: &str) {
    let _ = with_conn(|conn| {
        conn.execute(
            "INSERT INTO meta(key,value) VALUES(?1,?2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )
    });
}

pub fn get_meta(key: &str) -> Option<String> {
    with_conn(|conn| {
        conn.query_row("SELECT value FROM meta WHERE key=?1", params![key], |r| {
            r.get::<_, String>(0)
        })
        .optional()
        .ok()
        .flatten()
    })
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(account: &str, day: &str, model: &str, cost: i64, requests: i64) -> UsageDailyRow {
        UsageDailyRow {
            account_id: account.into(),
            day: day.into(),
            user_type: "service_account".into(),
            user_id: "svc_1".into(),
            user_name: "claude".into(),
            provider: "opencode-go".into(),
            model: model.into(),
            requests,
            input_tokens: 100,
            output_tokens: 20,
            cache_read_tokens: 0,
            cache_write_5m_tokens: 0,
            cache_write_1h_tokens: 0,
            cost_micro_cents: cost,
        }
    }

    /// 同一账号同一主键重复 upsert 必须更新而非新增（否则每 5 秒的增量同步会把表撑爆）；
    /// 不同账号的同一天同一模型必须各自独立成行，不能互相覆盖。
    #[test]
    fn upsert_is_idempotent_and_scoped_by_account() {
        use_memory_db_for_test();
        assert_eq!(upsert_rows(&[row("acct_1", "2026-10-01", "m1", 100, 1)]), 1);
        assert_eq!(upsert_rows(&[row("acct_1", "2026-10-02", "m1", 200, 2)]), 1);
        // 同账号同一天同一模型：应更新那一行
        assert_eq!(upsert_rows(&[row("acct_1", "2026-10-02", "m1", 350, 7)]), 1);
        // 另一个账号的同一天同一模型：独立成行，不能覆盖 acct_1
        assert_eq!(upsert_rows(&[row("acct_2", "2026-10-02", "m1", 500, 5)]), 1);

        let (count, cost) = stats_summary();
        assert_eq!(count, 3, "重复 upsert 不应新增行；不同账号的数据必须隔离");
        assert_eq!(cost, 950, "花费应被覆盖而不是累加");
    }
}

// ──────────────── 逐条请求日志（会话授权后） ────────────────

/// 写入逐条日志（按 `(account_id, id)` 幂等）。
pub fn insert_request_logs(logs: &[crate::opencode::RequestLog], account_id: &str) -> usize {
    if logs.is_empty() {
        return 0;
    }
    let mut written = 0usize;
    with_conn(|conn| {
        let tx = conn.unchecked_transaction().ok();
        for l in logs {
            let n = conn.execute(
                "INSERT OR REPLACE INTO request_log
                   (account_id, id, started_at_ms, finished_at_ms, duration_ms, provider, model,
                    user_id, user_type, input_tokens, output_tokens, reasoning_tokens,
                    cache_read_tokens, cache_write_tokens, cache_write_1h_tokens,
                    cost_micro_cents, status_code)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
                params![
                    account_id, l.id, l.started_at_ms, l.finished_at_ms, l.duration_ms, l.provider, l.model,
                    l.user_id, l.user_type, l.input_tokens, l.output_tokens, l.reasoning_tokens,
                    l.cache_read_tokens, l.cache_write_tokens, l.cache_write_1h_tokens,
                    l.cost_micro_cents, l.status_code
                ],
            );
            if n.is_ok() {
                written += 1;
            }
        }
        if let Some(tx) = tx {
            let _ = tx.commit();
        }
    });
    written
}

/// (条数, 最早 started_at_ms, 最晚 started_at_ms)
pub fn request_log_stats() -> (i64, i64, i64) {
    with_conn(|conn| {
        conn.query_row(
            "SELECT COUNT(*), COALESCE(MIN(started_at_ms),0), COALESCE(MAX(started_at_ms),0) FROM request_log",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap_or((0, 0, 0))
    })
    .unwrap_or((0, 0, 0))
}

/// 某账号的 (条数, 最晚 started_at_ms)，用于账号列表。
pub fn account_row_stats(account_id: &str) -> (i64, i64) {
    with_conn(|conn| {
        conn.query_row(
            "SELECT COUNT(*), COALESCE(MAX(started_at_ms),0) FROM request_log WHERE account_id = ?1",
            params![account_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or((0, 0))
    })
    .unwrap_or((0, 0))
}

/// 删除某账号的全部本地数据（删除账号时用）。返回删除的行数。
pub fn purge_account(account_id: &str) -> usize {
    with_conn(|conn| {
        let mut n = conn
            .execute("DELETE FROM request_log WHERE account_id = ?1", params![account_id])
            .unwrap_or(0);
        n += conn
            .execute("DELETE FROM usage_daily WHERE account_id = ?1", params![account_id])
            .unwrap_or(0);
        let _ = conn.execute(
            "DELETE FROM meta WHERE key LIKE '%:' || ?1",
            params![account_id],
        );
        n
    })
    .unwrap_or(0)
}

/// 把逐条日志聚合成按天行（供 usage_daily / 日志表展示）。
pub fn request_logs_to_daily() -> Vec<UsageDailyRow> {
    let mut out = Vec::new();
    with_conn(|conn| {
        let mut stmt = match conn.prepare(
            "SELECT account_id,
                    date(started_at_ms/1000,'unixepoch') AS day, user_type, user_id, provider, model,
                    COUNT(*), COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0),
                    COALESCE(SUM(cache_read_tokens),0), COALESCE(SUM(cache_write_tokens),0),
                    COALESCE(SUM(cache_write_1h_tokens),0), COALESCE(SUM(cost_micro_cents),0)
             FROM request_log GROUP BY account_id, day, user_type, user_id, provider, model",
        ) {
            Ok(s) => s,
            Err(_) => return,
        };
        let mapped = stmt.query_map([], |row| {
            Ok(UsageDailyRow {
                account_id: row.get(0)?,
                day: row.get(1)?,
                user_type: row.get(2)?,
                user_id: row.get(3)?,
                user_name: String::new(),
                provider: row.get(4)?,
                model: row.get(5)?,
                requests: row.get(6)?,
                input_tokens: row.get(7)?,
                output_tokens: row.get(8)?,
                cache_read_tokens: row.get(9)?,
                cache_write_5m_tokens: row.get(10)?,
                cache_write_1h_tokens: row.get(11)?,
                cost_micro_cents: row.get(12)?,
            })
        });
        if let Ok(rows) = mapped {
            for r in rows.flatten() {
                out.push(r);
            }
        }
    });
    out
}

/// 图表用：逐条日志按 `bucket_ms` 分桶聚合（秒/分钟粒度）。
#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct SeriesPoint {
    pub ts_ms: i64,
    pub requests: i64,
    pub error_requests: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cost_micro_cents: i64,
}

impl SeriesPoint {
    pub fn total_tokens(&self) -> i64 {
        self.input_tokens + self.output_tokens + self.cache_read_tokens
    }
}

pub fn request_series(since_ms: i64, until_ms: Option<i64>, bucket_ms: i64) -> Vec<SeriesPoint> {
    let bucket_ms = bucket_ms.max(1000);
    let mut out = Vec::new();
    with_conn(|conn| {
        let mut stmt = match conn.prepare(
            "SELECT (started_at_ms/?3)*?3 AS b, COUNT(*),
                    COALESCE(SUM(CASE WHEN status_code >= 400 THEN 1 ELSE 0 END),0),
                    COALESCE(SUM(input_tokens),0),
                    COALESCE(SUM(output_tokens),0), COALESCE(SUM(cache_read_tokens),0),
                    COALESCE(SUM(cost_micro_cents),0)
             FROM request_log
             WHERE started_at_ms >= ?1 AND (?2 IS NULL OR started_at_ms < ?2)
             GROUP BY b ORDER BY b ASC",
        ) {
            Ok(s) => s,
            Err(_) => return,
        };
        let mapped = stmt.query_map(params![since_ms, until_ms, bucket_ms], |row| {
            Ok(SeriesPoint {
                ts_ms: row.get(0)?,
                requests: row.get(1)?,
                error_requests: row.get(2)?,
                input_tokens: row.get(3)?,
                output_tokens: row.get(4)?,
                cache_read_tokens: row.get(5)?,
                cost_micro_cents: row.get(6)?,
            })
        });
        if let Ok(rows) = mapped {
            for r in rows.flatten() {
                out.push(r);
            }
        }
    });
    out
}

pub fn prune_request_logs(before_ms: i64) {
    let _ = with_conn(|conn| {
        conn.execute("DELETE FROM request_log WHERE started_at_ms < ?1", params![before_ms])
    });
}

// ──────────────── 每模型聚合 / RPM / 逐条分页 ────────────────

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct ModelUsage {
    pub model: String,
    pub requests: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cost_micro_cents: i64,
}

/// 某个时间点之后，按模型聚合。
pub fn model_usage_since(since_ms: i64, account_id: &str) -> Vec<ModelUsage> {
    let mut out = Vec::new();
    with_conn(|conn| {
        let mut stmt = match conn.prepare(
            "SELECT model, COUNT(*), COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0),
                    COALESCE(SUM(cache_read_tokens),0), COALESCE(SUM(cost_micro_cents),0)
             FROM request_log
             WHERE started_at_ms >= ?1 AND (?2 = '' OR account_id = ?2)
             GROUP BY model ORDER BY 6 DESC",
        ) {
            Ok(s) => s,
            Err(_) => return,
        };
        let mapped = stmt.query_map(params![since_ms, account_id], |row| {
            Ok(ModelUsage {
                model: row.get(0)?,
                requests: row.get(1)?,
                input_tokens: row.get(2)?,
                output_tokens: row.get(3)?,
                cache_read_tokens: row.get(4)?,
                cost_micro_cents: row.get(5)?,
            })
        });
        if let Ok(rows) = mapped {
            for r in rows.flatten() {
                out.push(r);
            }
        }
    });
    out
}

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct RpmStats {
    /// 最近一个完整分钟的请求数
    pub current: i64,
    /// 窗口内每分钟请求数的峰值
    pub peak: i64,
    /// 窗口内平均每分钟请求数
    pub avg: f64,
    /// 最近 5 分钟请求数
    pub last_5m: i64,
    /// 窗口内 429（被限流）次数
    pub throttled_429: i64,
    /// 窗口内总请求数
    pub total: i64,
}

/// 从逐条日志计算实际 RPM（官方未公开 RPM 上限，这里给观测值）。
pub fn rpm_stats(window_ms: i64) -> RpmStats {
    let now = chrono::Utc::now().timestamp_millis();
    let since = now - window_ms.max(60_000);
    let minute_floor = (now / 60_000) * 60_000;
    let mut per_minute: Vec<(i64, i64)> = Vec::new();
    let mut throttled = 0i64;
    with_conn(|conn| {
        if let Ok(mut stmt) = conn.prepare(
            "SELECT (started_at_ms/60000)*60000 AS m, COUNT(*) FROM request_log
             WHERE started_at_ms >= ?1 GROUP BY m ORDER BY m",
        ) {
            let mapped = stmt.query_map(params![since], |row| Ok((row.get(0)?, row.get(1)?)));
            if let Ok(rows) = mapped {
                for r in rows.flatten() {
                    per_minute.push(r);
                }
            }
        }
        throttled = conn
            .query_row(
                "SELECT COUNT(*) FROM request_log WHERE started_at_ms >= ?1 AND status_code = 429",
                params![since],
                |r| r.get(0),
            )
            .unwrap_or(0);
    });

    let total: i64 = per_minute.iter().map(|(_, c)| *c).sum();
    let peak = per_minute.iter().map(|(_, c)| *c).max().unwrap_or(0);
    // 当前 = 正在进行的这一分钟（未走完，随同步刷新）
    let current = per_minute
        .iter()
        .find(|(m, _)| *m == minute_floor)
        .map(|(_, c)| *c)
        .unwrap_or(0);
    let last_5m: i64 = per_minute
        .iter()
        .filter(|(m, _)| *m > now - 5 * 60_000)
        .map(|(_, c)| *c)
        .sum();
    let minutes = (window_ms.max(60_000) as f64) / 60_000.0;
    RpmStats {
        current,
        peak,
        avg: (total as f64 / minutes * 10.0).round() / 10.0,
        last_5m,
        throttled_429: throttled,
        total,
    }
}

/// 逐条日志分页（按时间倒序）。
pub fn query_request_logs(
    since_ms: Option<i64>,
    account_id: Option<&str>,
    page: u32,
    page_size: u32,
) -> (Vec<crate::opencode::RequestLog>, u32) {
    let page = page.max(1);
    let page_size = page_size.clamp(1, 200);
    let offset = (page - 1) * page_size;
    let mut out = Vec::new();
    let total = with_conn(|conn| {
        let total: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM request_log
                 WHERE (?1 IS NULL OR started_at_ms >= ?1) AND (?2 IS NULL OR account_id = ?2)",
                params![since_ms, account_id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let sql = format!(
            "SELECT account_id, id, started_at_ms, finished_at_ms, duration_ms, provider, model,
                    user_id, user_type, input_tokens, output_tokens, reasoning_tokens,
                    cache_read_tokens, cache_write_tokens, cache_write_1h_tokens,
                    cost_micro_cents, status_code
             FROM request_log
             WHERE (?1 IS NULL OR started_at_ms >= ?1) AND (?2 IS NULL OR account_id = ?2)
             ORDER BY started_at_ms DESC LIMIT {page_size} OFFSET {offset}"
        );
        if let Ok(mut stmt) = conn.prepare(&sql) {
            let mapped = stmt.query_map(params![since_ms, account_id], |row| {
                Ok(crate::opencode::RequestLog {
                    account_id: row.get(0)?,
                    id: row.get(1)?,
                    started_at_ms: row.get(2)?,
                    finished_at_ms: row.get(3)?,
                    duration_ms: row.get(4)?,
                    provider: row.get(5)?,
                    model: row.get(6)?,
                    user_id: row.get(7)?,
                    user_type: row.get(8)?,
                    input_tokens: row.get(9)?,
                    output_tokens: row.get(10)?,
                    reasoning_tokens: row.get(11)?,
                    cache_read_tokens: row.get(12)?,
                    cache_write_tokens: row.get(13)?,
                    cache_write_1h_tokens: row.get(14)?,
                    cost_micro_cents: row.get(15)?,
                    status_code: row.get(16)?,
                    ..Default::default()
                })
            });
            if let Ok(rows) = mapped {
                for r in rows.flatten() {
                    out.push(r);
                }
            }
        }
        total
    });
    (out, total.unwrap_or(0) as u32)
}

// ──────────────── 窗口统计 / 当前分钟 ────────────────

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct WindowStats {
    pub requests: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    /// 窗口内花费（microCents）
    pub cost_micro_cents: i64,
    /// 有请求的分钟数
    pub active_minutes: i64,
    /// 窗口总分钟数
    pub window_minutes: i64,
}

/// 指定窗口内的汇总（用于「词元数明细 / 时间窗口内」两张卡）。
pub fn window_stats(since_ms: i64, until_ms: Option<i64>) -> WindowStats {
    let now = chrono::Utc::now().timestamp_millis();
    let end = until_ms.unwrap_or(now);
    let mut out = WindowStats::default();
    with_conn(|conn| {
        let (req, input, output, cache, cost, active, min_started): (i64, i64, i64, i64, i64, i64, i64) = conn
            .query_row(
                "SELECT COUNT(*), COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0),
                        COALESCE(SUM(cache_read_tokens),0), COALESCE(SUM(cost_micro_cents),0),
                        COUNT(DISTINCT (started_at_ms/60000)),
                        COALESCE(MIN(started_at_ms),0)
                 FROM request_log
                 WHERE (?1 = 0 OR started_at_ms >= ?1) AND (?2 IS NULL OR started_at_ms < ?2)",
                params![since_ms, until_ms],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
            )
            .unwrap_or((0, 0, 0, 0, 0, 0, 0));
        let start = if since_ms > 0 { since_ms } else { min_started };
        let span = if start > 0 { (end - start).max(0) } else { 0 };
        out = WindowStats {
            requests: req,
            input_tokens: input,
            output_tokens: output,
            cache_read_tokens: cache,
            cost_micro_cents: cost,
            active_minutes: active,
            window_minutes: span / 60000,
        };
    });
    out
}

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct MinuteTokens {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
}

/// 当前这一分钟（未走完）的词元数（用于「当前分钟」卡），随每次刷新增长。
pub fn current_minute_tokens() -> MinuteTokens {
    let now = chrono::Utc::now().timestamp_millis();
    let minute_floor = (now / 60_000) * 60_000;
    let mut out = MinuteTokens::default();
    with_conn(|conn| {
        out = conn
            .query_row(
                "SELECT COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0),
                        COALESCE(SUM(cache_read_tokens),0)
                 FROM request_log WHERE started_at_ms >= ?1",
                params![minute_floor],
                |r| Ok(MinuteTokens {
                    input_tokens: r.get(0)?,
                    output_tokens: r.get(1)?,
                    cache_read_tokens: r.get(2)?,
                }),
            )
            .unwrap_or_default();
    });
    out
}
