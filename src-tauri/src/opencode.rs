//! OpenCode 控制台 API 客户端。
//!
//! 基址默认 `https://opencode.ai/console/api`；程序只用**控制台登录会话**
//! （`SessionClient`：会话 Cookie + `x-org-id` + 浏览器 UA/Referer）访问接口，
//! Service API Key（`Authorization: Bearer oc_sk_...`）已被废除：
//! 它无法访问 `/request-logs`（恒 403），而逐条日志是本程序唯一的数据来源。

use serde::Serialize;
use serde_json::Value;
use std::time::Duration;

pub const DEFAULT_BASE_URL: &str = "https://opencode.ai/console/api";

/// 仿 Chrome 的 UA——会话接口可能校验来源，用默认的 reqwest UA 容易吃 403。
pub const BROWSER_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";

/// 从 JSON 里取整数：API 把大整数统一序列化成字符串（如 "1200000000"），
/// 少数场景又是数字，两种都要兼容。
fn num(v: &Value, key: &str) -> i64 {
    match v.get(key) {
        Some(Value::String(s)) => s.parse::<i64>().unwrap_or(0),
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0),
        _ => 0,
    }
}

fn s(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .to_string()
}

fn opt_s(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(|x| x.as_str()).map(|x| x.to_string())
}

/// 一个额度计（5 小时 / 周 / 月）
#[derive(Debug, Clone, Serialize, Default)]
pub struct Meter {
    pub starts_at: Option<String>,
    pub resets_at: Option<String>,
    pub limit_micro_cents: i64,
    pub used_micro_cents: i64,
}

impl Meter {
    fn from_json(v: &Value) -> Self {
        Self {
            starts_at: opt_s(v, "startsAt"),
            resets_at: opt_s(v, "resetsAt"),
            limit_micro_cents: num(v, "limitMicroCents"),
            used_micro_cents: num(v, "usedMicroCents"),
        }
    }
}

/// Go / Go Plus 订阅与实时额度
#[derive(Debug, Clone, Serialize, Default)]
pub struct Quota {
    /// go / go-plus / 其他
    pub product: String,
    pub plan_name: String,
    pub cancel_at_period_end: bool,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub five_hour: Meter,
    pub week: Meter,
    pub month: Meter,
    /// 升级到 Go Plus 的价格（microCents），无则为 None
    pub upgrade_price_micro_cents: Option<i64>,
}

impl Quota {
    pub fn from_json(v: &Value) -> Self {
        let access = v.get("access").cloned().unwrap_or(Value::Null);
        let meters = access.get("meters").cloned().unwrap_or(Value::Null);
        let product = s(v, "product");
        let plan_name = match product.as_str() {
            "go" => "Go 订阅计划",
            "go-plus" => "Go Plus 订阅计划",
            _ => "Go 订阅计划",
        }
        .to_string();
        let upgrade = v
            .get("upgradePrice")
            .map(|u| num(u, "amountMicroCents"))
            .filter(|n| *n > 0);
        Self {
            product,
            plan_name,
            cancel_at_period_end: v
                .get("cancelAtPeriodEnd")
                .and_then(|x| x.as_bool())
                .unwrap_or(false),
            starts_at: opt_s(&access, "startsAt"),
            ends_at: opt_s(&access, "endsAt"),
            five_hour: Meter::from_json(meters.get("fiveHour").unwrap_or(&Value::Null)),
            week: Meter::from_json(meters.get("week").unwrap_or(&Value::Null)),
            month: Meter::from_json(meters.get("month").unwrap_or(&Value::Null)),
            upgrade_price_micro_cents: upgrade,
        }
    }
}



/// 按天/小时的成本点
#[derive(Debug, Clone, Serialize, Default)]
pub struct CostPoint {
    /// `2026-10-02`（day）或 `2026-10-02T07:00:00Z`（hour）
    pub date: String,
    pub total_cost_micro_cents: i64,
    pub total_tokens: i64,
    pub total_requests: i64,
}

/// v2 用量导出的一行（按天 × 用户 × provider × model 汇总）
#[derive(Debug, Clone, Serialize, Default)]
pub struct UsageDailyRow {
    pub account_id: String,
    pub day: String,
    pub user_type: String,
    pub user_id: String,
    pub user_name: String,
    pub provider: String,
    pub model: String,
    pub requests: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_5m_tokens: i64,
    pub cache_write_1h_tokens: i64,
    pub cost_micro_cents: i64,
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n).collect::<String>() + "…"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn num_accepts_string_and_number() {
        let v = serde_json::json!({"a":"1200000000","b":42});
        assert_eq!(num(&v, "a"), 1_200_000_000);
        assert_eq!(num(&v, "b"), 42);
        assert_eq!(num(&v, "missing"), 0);
    }
}


// ──────────────── 会话（登录）客户端 ────────────────
//
// Service API Key 无法访问 `/request-logs`（恒 403）。逐条日志只能靠
// 控制台网页登录后的会话 Cookie（`__Host-console_session`）调用。
// 鉴权：`Cookie: __Host-console_session=...`，并带上 `x-org-id`。

/// 一条逐条请求日志（`/request-logs` 的 items）
#[derive(Debug, Clone, Serialize, Default)]
pub struct RequestLog {
    pub account_id: String,
    pub id: String,
    pub started_at_ms: i64,
    pub finished_at_ms: i64,
    pub duration_ms: i64,
    pub category: String,
    pub provider: String,
    pub model: String,
    pub user_id: String,
    pub user_type: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub reasoning_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub cache_write_1h_tokens: i64,
    pub cost_micro_cents: i64,
    pub status_code: i64,
}

impl RequestLog {
    pub fn from_json(v: &Value) -> Self {
        let started = ts_millis(v, "startedAt");
        let finished = ts_millis(v, "finishedAt");
        let service_id = v
            .get("serviceAccountID")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let user_id = if !service_id.is_empty() {
            service_id.clone()
        } else {
            s(v, "userID")
        };
        let user_type = if service_id.is_empty() { "member" } else { "service_account" }.to_string();
        Self {
            account_id: String::new(),
            id: s(v, "id"),
            started_at_ms: started,
            finished_at_ms: finished,
            duration_ms: num(v, "durationMs"),
            category: s(v, "category"),
            provider: s(v, "provider"),
            model: {
                let m = s(v, "model");
                if m.is_empty() { s(v, "requestedModel") } else { m }
            },
            user_id,
            user_type,
            input_tokens: num(v, "inputTokens"),
            output_tokens: num(v, "outputTokens"),
            reasoning_tokens: num(v, "reasoningTokens"),
            cache_read_tokens: num(v, "cacheReadTokens"),
            cache_write_tokens: num(v, "cacheWriteTokens"),
            cache_write_1h_tokens: num(v, "cacheWrite1hTokens"),
            // 控制台统一用 microCents；若某天变成美元可在此换算
            cost_micro_cents: num(v, "cost"),
            status_code: num(v, "statusCode"),
        }
    }
}

/// 兼容毫秒数字 / RFC3339 字符串 / 秒级数字
fn ts_millis(v: &Value, key: &str) -> i64 {
    match v.get(key) {
        Some(Value::Number(n)) => {
            let raw = n.as_i64().unwrap_or(0);
            if raw > 1_000_000_000_000 { raw } else { raw * 1000 }
        }
        Some(Value::String(s)) => {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
                return dt.timestamp_millis();
            }
            s.parse::<i64>().map(|n| if n > 1_000_000_000_000 { n } else { n * 1000 }).unwrap_or(0)
        }
        _ => 0,
    }
}

/// 一页逐条日志
#[derive(Debug, Clone, Serialize, Default)]
pub struct RequestLogPage {
    pub items: Vec<RequestLog>,
    pub next_cursor: Option<String>,
    pub until: Option<i64>,
    pub retention_days: i64,
}

/// 用控制台会话 Cookie 调用的客户端。
#[derive(Clone)]
pub struct SessionClient {
    http: reqwest::Client,
    base_url: String,
    cookie: String,
    org_id: Option<String>,
}

impl SessionClient {
    pub fn new(cookie: impl Into<String>, base_url: impl Into<String>, org_id: Option<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(BROWSER_UA)
            .build()
            .unwrap_or_default();
        let mut base = base_url.into();
        while base.ends_with('/') {
            base.pop();
        }
        Self {
            http,
            base_url: base,
            cookie: cookie.into(),
            org_id,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// 仿浏览器请求：带会话 Cookie、浏览器 UA、Referer/Origin，以及工作区头。
    fn req(&self, path: &str) -> reqwest::RequestBuilder {
        // 存的是完整 Cookie 头（可能含多个 cookie）；兼容旧版只存 session 值的情况。
        let cookie = if self.cookie.contains('=') {
            self.cookie.clone()
        } else {
            format!("__Host-console_session={}", self.cookie)
        };
        let mut b = self
            .http
            .get(self.url(path))
            .header("Cookie", cookie)
            .header("Accept", "application/json, text/plain, */*")
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Referer", "https://opencode.ai/console/")
            .header("Origin", "https://opencode.ai");
        if let Some(org) = &self.org_id {
            b = b.header("x-org-id", org);
        }
        b
    }

    async fn get_json(&self, path: &str) -> Result<Value, String> {
        let resp = self
            .req(path)
            .send()
            .await
            .map_err(|e| format!("网络请求失败：{e}"))?;
        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err("登录已失效（401），请重新授权".into());
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!(
                "{} 返回 HTTP {}：{}（x-org-id: {}）",
                path,
                status.as_u16(),
                truncate(&body, 200),
                if self.org_id.is_some() { "有" } else { "无" }
            ));
        }
        resp.json::<Value>().await.map_err(|e| format!("解析响应失败：{e}"))
    }

    /// 拉取一页逐条日志。
    pub async fn request_logs_page(
        &self,
        since_ms: i64,
        until_ms: Option<i64>,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<RequestLogPage, String> {
        let mut q = format!(
            "/request-logs?since={since_ms}&category=inference&limit={}",
            limit.clamp(1, 100)
        );
        if let Some(u) = until_ms {
            q.push_str(&format!("&until={u}"));
        }
        if let Some(c) = cursor {
            if !c.is_empty() {
                q.push_str(&format!("&cursor={c}"));
            }
        }
        let v = self.get_json(&q).await?;
        let items = v
            .get("items")
            .and_then(|x| x.as_array())
            .map(|arr| arr.iter().map(RequestLog::from_json).collect())
            .unwrap_or_default();
        Ok(RequestLogPage {
            items,
            next_cursor: v.get("nextCursor").and_then(|x| x.as_str()).map(|s| s.to_string()),
            until: v.get("until").and_then(|x| x.as_i64()),
            retention_days: num(&v, "retentionDays"),
        })
    }

    /// 当前工作区（用于拿 org id）
    pub async fn orgs_current(&self) -> Result<Value, String> {
        self.get_json("/orgs/current").await
    }

    /// 工作区列表（备用）
    pub async fn orgs_list(&self) -> Result<Value, String> {
        self.get_json("/orgs").await
    }

    /// 当前用户信息（备用）
    pub async fn user_info(&self) -> Result<Value, String> {
        self.get_json("/user").await
    }

    /// 会话查额度。
    pub async fn go_status(&self) -> Result<Quota, String> {
        let v = self.get_json("/go/status").await?;
        Ok(Quota::from_json(&v))
    }

    /// 会话查成本曲线
    pub async fn cost_by_day(&self, range: &str, bucket: &str) -> Result<Vec<CostPoint>, String> {
        let v = self
            .get_json(&format!("/usage/cost-by-day?range={range}&bucket={bucket}"))
            .await?;
        let arr = v.as_array().cloned().unwrap_or_default();
        Ok(arr
            .iter()
            .map(|p| CostPoint {
                date: s(p, "date"),
                total_cost_micro_cents: num(p, "totalCostMicroCents"),
                total_tokens: num(p, "totalTokens"),
                total_requests: num(p, "totalRequests"),
            })
            .collect())
    }
}
