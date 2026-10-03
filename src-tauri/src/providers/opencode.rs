//! OpenCode 控制台 API 客户端。
//!
//! 基址默认 `https://opencode.ai/console/api`；程序只用**控制台登录会话**
//! （`SessionClient`：会话 Cookie + `x-org-id` + 浏览器 UA/Referer）访问接口，
//! Service API Key（`Authorization: Bearer oc_sk_...`）已被废除：
//! 它无法访问 `/request-logs`（恒 403），而逐条日志是本程序唯一的数据来源。

use super::{Credentials, Identity, ProviderId, UsageProvider};
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

/// 取浮点数（字符串数字也兼容）。
fn num_f64(v: &Value, key: &str) -> f64 {
    match v.get(key) {
        Some(Value::String(s)) => s.trim().parse::<f64>().unwrap_or(0.0),
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        _ => 0.0,
    }
}

/// `/request-logs` 的 `cost` 是**美元浮点**（实测如 `0.00113096`），入库统一成 microCents。
/// 1 美元 = 1e8 microCents（与前端 `fmtUsd` 一致）；若某天改回 microCents 整数，按量级兜底。
fn cost_micro_cents(v: &Value) -> i64 {
    let raw = num_f64(v, "cost");
    if raw <= 0.0 {
        return 0;
    }
    if raw >= 100.0 {
        raw.round() as i64
    } else {
        (raw * 1e8).round() as i64
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

    /// `/request-logs` 的 cost 是美元浮点：必须换算成 microCents，否则金额全是 0。
    #[test]
    fn request_log_cost_is_parsed_as_usd() {
        let log = RequestLog::from_json(&serde_json::json!({
            "id": "x", "cost": 0.00113096
        }));
        assert_eq!(log.cost_micro_cents, 113_096, "0.00113096 美元 = 113096 microCents");
        let stringy = RequestLog::from_json(&serde_json::json!({"cost": "0.5"}));
        assert_eq!(stringy.cost_micro_cents, 50_000_000, "字符串金额也要认");
        let micro = RequestLog::from_json(&serde_json::json!({"cost": 113096}));
        assert_eq!(micro.cost_micro_cents, 113_096, "已是 microCents 时按量级兜底");
        let none = RequestLog::from_json(&serde_json::json!({"cost": null}));
        assert_eq!(none.cost_micro_cents, 0);
    }

    #[test]
    fn extract_org_id_finds_workspace_id() {
        let v = serde_json::json!({"data": {"id": "org_abc123"}});
        assert_eq!(extract_org_id(&v).as_deref(), Some("org_abc123"));
        let v = serde_json::json!({"items": [{"id": "wrk_9"}]});
        assert_eq!(extract_org_id(&v).as_deref(), Some("wrk_9"));
        assert_eq!(extract_org_id(&serde_json::json!({"x": 1})), None);
    }

    #[test]
    fn extract_label_prefers_email_and_respects_depth() {
        let v = serde_json::json!({"data": {"user": {"email": "a@b.c", "name": "A"}}});
        assert_eq!(extract_label(&v).as_deref(), Some("a@b.c"));
        // 深度 > 2 的字段不再深入
        let deep = serde_json::json!({"a": {"b": {"c": {"name": "too deep"}}}});
        assert_eq!(extract_label(&deep), None);
        assert_eq!(extract_label(&serde_json::json!({"name": "  "})), None);
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
            cost_micro_cents: cost_micro_cents(v),
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

// ──────────────── 登录探测辅助 ────────────────

/// 在任意 JSON 里递归找形如 `org_...` 的字符串（工作区 id）。
pub fn extract_org_id(v: &serde_json::Value) -> Option<String> {
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
pub fn extract_label(v: &serde_json::Value) -> Option<String> {
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

// ──────────────── 平台接口实现 ────────────────

/// OpenCode 平台实现：凭据是控制台登录会话（Cookie + 工作区 id）。
pub struct OpenCodeProvider;

impl OpenCodeProvider {
    /// 按平台凭据建一个会话客户端（`org_id` 为空时不带 `x-org-id`）。
    fn session(&self, creds: &Credentials) -> SessionClient {
        let org = if creds.org_id.trim().is_empty() {
            None
        } else {
            Some(creds.org_id.clone())
        };
        SessionClient::new(creds.cookie.clone(), crate::base_url(), org)
    }
}

impl UsageProvider for OpenCodeProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Opencode
    }

    fn login_url(&self) -> &'static str {
        "https://opencode.ai/console/"
    }

    async fn quota(&self, creds: &Credentials) -> Result<Quota, String> {
        self.session(creds).go_status().await
    }

    async fn request_logs(
        &self,
        creds: &Credentials,
        since_ms: i64,
        until_ms: i64,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<RequestLogPage, String> {
        self.session(creds)
            .request_logs_page(since_ms, Some(until_ms), cursor, limit)
            .await
    }

    async fn probe(&self, creds: &Credentials) -> Identity {
        let client = self.session(creds);
        let mut label = None;
        let mut org_id = None;
        for res in [
            client.orgs_current().await,
            client.orgs_list().await,
            client.user_info().await,
        ] {
            if let Ok(v) = res {
                if label.is_none() {
                    label = extract_label(&v);
                }
                if org_id.is_none() {
                    org_id = extract_org_id(&v);
                }
                if label.is_some() && org_id.is_some() {
                    break;
                }
            }
        }
        Identity { label, org_id }
    }

    async fn cost_by_day(
        &self,
        creds: &Credentials,
        range: &str,
        bucket: &str,
    ) -> Result<Vec<CostPoint>, String> {
        self.session(creds).cost_by_day(range, bucket).await
    }
}
