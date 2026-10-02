//! OpenCode 控制台 API 客户端。
//!
//! 基址默认 `https://opencode.ai/console/api`，鉴权统一用
//! `Authorization: Bearer <oc_sk_...>`（见 docs/opencode-api-research.md）。
//!
//! 只有「全部（all）」权限的 Service API Key 才能读取用量与额度；
//! 「仅推理（inference-only）」的 Key 调用这里会得到 401/403。

use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
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

/// 用量汇总
#[derive(Debug, Clone, Serialize, Default)]
pub struct Summary {
    pub total_requests: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_5m_tokens: i64,
    pub cache_write_1h_tokens: i64,
    pub total_cost_micro_cents: i64,
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

impl Summary {
    pub fn from_json(v: &Value) -> Self {
        Self {
            total_requests: num(v, "totalRequests"),
            input_tokens: num(v, "totalInputTokens"),
            output_tokens: num(v, "totalOutputTokens"),
            cache_read_tokens: num(v, "totalCacheReadTokens"),
            cache_write_5m_tokens: num(v, "totalCacheWrite5mTokens"),
            cache_write_1h_tokens: num(v, "totalCacheWrite1hTokens"),
            total_cost_micro_cents: num(v, "totalCostMicroCents"),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.total_requests == 0
            && self.input_tokens == 0
            && self.output_tokens == 0
            && self.cache_read_tokens == 0
            && self.total_cost_micro_cents == 0
    }

    /// 词元总量（输入 + 输出 + 缓存读），用于图表
    pub fn total_tokens(&self) -> i64 {
        self.input_tokens + self.output_tokens + self.cache_read_tokens
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

/// API 客户端
#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl Client {
    pub fn with_base(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("opencode-monitor/0.1")
            .build()
            .unwrap_or_default();
        let mut base = base_url.into();
        while base.ends_with('/') {
            base.pop();
        }
        Self {
            http,
            base_url: base,
            api_key: api_key.into(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// 发请求并解析 JSON。401/403 给出明确的中文提示（权限问题是最常见的坑）。
    async fn get_json(&self, path: &str) -> Result<Value, String> {
        let resp = self
            .http
            .get(self.url(path))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| format!("网络请求失败：{e}"))?;
        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err("API Key 无效或已撤销（401）".into());
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err("该 API Key 无权访问此接口（403）；请确认创建时选择了「全部」权限".into());
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("接口返回 HTTP {}：{}", status.as_u16(), truncate(&body, 200)));
        }
        resp.json::<Value>()
            .await
            .map_err(|e| format!("解析响应失败：{e}"))
    }

    /// 订阅 + 实时额度。未订阅 Go 时该接口可能 404，转成可读提示。
    pub async fn go_status(&self) -> Result<Quota, String> {
        let v = self.get_json("/go/status").await?;
        Ok(Quota::from_json(&v))
    }

    /// 用量汇总。range: 24h / 7d / 30d / all
    pub async fn summary(&self, range: &str) -> Result<Summary, String> {
        let v = self.get_json(&format!("/usage/summary?range={range}")).await?;
        Ok(Summary::from_json(&v))
    }

    /// 自某个时刻以来的增量用量（实测 `since` 精确生效，可到秒）。
    /// 用它按采样间隔拉增量，就能得到秒级分辨率的序列。
    pub async fn summary_since(&self, since_iso: &str) -> Result<Summary, String> {
        let v = self
            .get_json(&format!("/usage/summary?since={since_iso}"))
            .await?;
        Ok(Summary::from_json(&v))
    }

    /// 成本曲线。bucket: day / hour
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

    /// 拉取 v1 逐条日志（仅未迁移到 v2 的组织可用，否则 403）。
    /// range: 24h / 7d / 30d。返回聚合后的按天行。
    pub async fn export_usage_v1(&self, range: &str) -> Result<Vec<UsageDailyRow>, String> {
        let resp = self
            .http
            .get(self.url(&format!("/v1/usage/export?scope=organization&range={range}")))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Accept", "text/csv")
            .send()
            .await
            .map_err(|e| format!("下载 v1 用量 CSV 失败：{e}"))?;
        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err("API Key 无效或已撤销（401）".into());
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err("v1 逐条日志不可用（403，组织已迁移 v2）".into());
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("v1 导出返回 HTTP {}：{}", status.as_u16(), truncate(&body, 200)));
        }
        let text = resp.text().await.map_err(|e| format!("读取 v1 CSV 失败：{e}"))?;
        Ok(parse_usage_v1_csv(&text))
    }

    /// 拉取 v2 按天汇总 CSV 并解析成结构化行。
    /// range: 7d / 30d / 90d（接口上限 90d）。
    pub async fn export_usage_v2(&self, range: &str) -> Result<Vec<UsageDailyRow>, String> {
        let resp = self
            .http
            .get(self.url(&format!("/v2/usage/export?range={range}")))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Accept", "text/csv")
            .send()
            .await
            .map_err(|e| format!("下载用量 CSV 失败：{e}"))?;
        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err("API Key 无效或已撤销（401）".into());
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err("无权导出用量（403）；请确认 Key 为「全部」权限".into());
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("导出接口返回 HTTP {}：{}", status.as_u16(), truncate(&body, 200)));
        }
        let text = resp
            .text()
            .await
            .map_err(|e| format!("读取 CSV 失败：{e}"))?;
        Ok(parse_usage_csv(&text))
    }
}

/// 解析 v2 用量 CSV。含表头，逐行字段顺序固定：
/// day,user_type,user_id,user_name,provider,model,requests,
/// input_tokens,output_tokens,cache_read_tokens,
/// cache_write_5m_tokens,cache_write_1h_tokens,cost_micro_cents
pub fn parse_usage_csv(text: &str) -> Vec<UsageDailyRow> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            continue;
        }
        if i == 0 && line.starts_with("day,") {
            continue; // 表头
        }
        let f = parse_csv_line(line);
        if f.len() < 13 {
            continue;
        }
        let p = |idx: usize| f.get(idx).cloned().unwrap_or_default();
        let n = |idx: usize| p(idx).parse::<i64>().unwrap_or(0);
        out.push(UsageDailyRow {
            day: p(0),
            user_type: p(1),
            user_id: p(2),
            user_name: p(3),
            provider: p(4),
            model: p(5),
            requests: n(6),
            input_tokens: n(7),
            output_tokens: n(8),
            cache_read_tokens: n(9),
            cache_write_5m_tokens: n(10),
            cache_write_1h_tokens: n(11),
            cost_micro_cents: n(12),
        });
    }
    out
}

/// 解析 v1 逐条日志 CSV，并按 `(day,user_type,user_id,provider,model)` 聚合成按天行。
///
/// v1 的表头对我们不可见（组织已迁移 v2，调用即 403），因此这里**按列名**
/// 而不是按位置取字段，并把常见的 camelCase / snake_case 统一归一化后再匹配。
/// 认不出的表头直接返回空，绝不 panic。
///
/// 别名兼容：
/// - 日期：day/date/createdAt/timestamp/time
/// - 用户：userId/serviceUserId/serviceAccountId + email/userName/name
/// - 词元：inputTokens/promptTokens、outputTokens/completionTokens、
///   cacheReadTokens/cachedTokens、cacheWrite5mTokens/cacheWriteTokens
/// - 花费：costMicroCents/costMicrocents/cost
pub fn parse_usage_v1_csv(text: &str) -> Vec<UsageDailyRow> {
    let mut lines = text.lines();
    let header_line = match lines.next() {
        Some(h) if !h.trim().is_empty() => h,
        _ => return Vec::new(),
    };
    let header: Vec<String> = parse_csv_line(header_line)
        .iter()
        .map(|h| normalize_col(h))
        .collect();

    // 聚合键 -> 累加值
    let mut agg: HashMap<(String, String, String, String, String), UsageDailyRow> = HashMap::new();

    for line in lines {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.trim().is_empty() {
            continue;
        }
        let fields = parse_csv_line(line);
        if fields.len() < 2 {
            continue;
        }
        let map: HashMap<&str, &str> = header
            .iter()
            .map(|s| s.as_str())
            .zip(fields.iter().map(|s| s.as_str()))
            .collect();

        let Some(day) = pick(&map, &["day", "date", "createdat", "timestamp", "time"])
            .and_then(to_day)
        else {
            continue;
        };
        let provider = pick(&map, &["provider"]).unwrap_or("opencode").to_string();
        let model = pick(&map, &["model", "modelid"]).unwrap_or("unknown").to_string();
        let service_id = pick(&map, &["serviceaccountid", "serviceuserid"]).filter(|s| !s.is_empty());
        let user_id = service_id
            .or_else(|| pick(&map, &["userid", "memberid"]))
            .unwrap_or("unknown")
            .to_string();
        let user_type = if service_id.is_some() { "service_account" } else { "member" }.to_string();
        let user_name = pick(&map, &["username", "email", "useremail", "name"])
            .unwrap_or("")
            .to_string();

        let requests = pick_i64(&map, &["requests", "requestcount"]).unwrap_or(1);
        let input_tokens = pick_i64(&map, &["inputtokens", "prompttokens"]).unwrap_or(0);
        let output_tokens = pick_i64(&map, &["outputtokens", "completiontokens"]).unwrap_or(0);
        let cache_read_tokens =
            pick_i64(&map, &["cachereadtokens", "cachedtokens"]).unwrap_or(0);
        let cache_write_5m_tokens =
            pick_i64(&map, &["cachewrite5mtokens", "cachewritetokens"]).unwrap_or(0);
        let cache_write_1h_tokens = pick_i64(&map, &["cachewrite1htokens"]).unwrap_or(0);
        let cost = pick_i64(&map, &["costmicrocents", "costmicrocent", "cost"]).unwrap_or(0);

        let key = (day.clone(), user_type.clone(), user_id.clone(), provider.clone(), model.clone());
        let e = agg.entry(key).or_insert_with(|| UsageDailyRow {
            day,
            user_type,
            user_id,
            user_name: user_name.clone(),
            provider,
            model,
            ..Default::default()
        });
        e.requests += requests;
        e.input_tokens += input_tokens;
        e.output_tokens += output_tokens;
        e.cache_read_tokens += cache_read_tokens;
        e.cache_write_5m_tokens += cache_write_5m_tokens;
        e.cache_write_1h_tokens += cache_write_1h_tokens;
        e.cost_micro_cents += cost;
    }

    let mut out: Vec<UsageDailyRow> = agg.into_values().collect();
    out.sort_by(|a, b| b.day.cmp(&a.day));
    out
}

/// 列名归一化：小写 + 去掉所有非字母数字字符。
/// 这样 `input_tokens` / `inputTokens` / `input tokens` 都变成 `inputtokens`。
fn normalize_col(s: &str) -> String {
    s.trim().to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect()
}

fn pick<'a>(map: &'a HashMap<&str, &'a str>, names: &[&str]) -> Option<&'a str> {
    for n in names {
        if let Some(v) = map.get(*n) {
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

fn pick_i64(map: &HashMap<&str, &str>, names: &[&str]) -> Option<i64> {
    pick(map, names).and_then(|s| s.trim().parse::<i64>().ok())
}

/// 把时间字符串归一化成 UTC 的 `YYYY-MM-DD`。
fn to_day(s: &str) -> Option<String> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s.trim()) {
        return Some(dt.with_timezone(&chrono::Utc).format("%Y-%m-%d").to_string());
    }
    let t = s.trim();
    let b = t.as_bytes();
    if b.len() >= 10 && b[4] == b'-' && b[7] == b'-' {
        return Some(t[..10].to_string());
    }
    None
}

/// 极简 CSV 单行解析：支持双引号包裹与 `""` 转义（用户名字段可能含逗号）。
fn parse_csv_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes => {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            }
            '"' => in_quotes = true,
            ',' if !in_quotes => {
                out.push(std::mem::take(&mut cur));
            }
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
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
    fn parse_csv_handles_header_quotes_and_commas() {
        let csv = "day,user_type,user_id,user_name,provider,model,requests,input_tokens,output_tokens,cache_read_tokens,cache_write_5m_tokens,cache_write_1h_tokens,cost_micro_cents\n\
2026-10-02,member,user_1,lengrenrushuang@gmail.com,opencode-go,deepseek-v4.1-flash,11,210225,4047,2041984,0,0,8017579\n\
2026-10-02,service_account,svc_1,\"claude, inc\",opencode-go,deepseek-v4.1-flash,231,400725,144689,32290560,0,0,39493787\n";
        let rows = parse_usage_csv(csv);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].day, "2026-10-02");
        assert_eq!(rows[0].cost_micro_cents, 8017579);
        assert_eq!(rows[1].user_name, "claude, inc", "引号内的逗号不应切分字段");
        assert_eq!(rows[1].requests, 231);
    }

    #[test]
    fn num_accepts_string_and_number() {
        let v = serde_json::json!({"a":"1200000000","b":42});
        assert_eq!(num(&v, "a"), 1_200_000_000);
        assert_eq!(num(&v, "b"), 42);
        assert_eq!(num(&v, "missing"), 0);
    }

    /// v1 表头对我们不可见，解析器按列名 + 别名匹配，并把逐条日志聚合成按天。
    #[test]
    fn parse_v1_aggregates_by_day_and_matches_camelcase_columns() {
        let csv = "createdAt,provider,model,serviceAccountId,userId,email,inputTokens,outputTokens,cacheReadTokens,cacheWrite5mTokens,cacheWrite1hTokens,costMicroCents\n\
2026-10-02T07:12:00Z,opencode-go,deepseek-v4.1-flash,svc_1,,claude@x.com,100,20,5,0,0,1234\n\
2026-10-02T08:30:00Z,opencode-go,deepseek-v4.1-flash,svc_1,,claude@x.com,200,30,7,0,0,2000\n\
2026-10-01T08:30:00Z,opencode-go,m2,,u1,u@x.com,10,1,0,0,0,50\n";
        let rows = parse_usage_v1_csv(csv);
        assert_eq!(rows.len(), 2, "同一天同一用户同一模型应聚合为一行");
        let r = rows.iter().find(|r| r.day == "2026-10-02").unwrap();
        assert_eq!(r.user_type, "service_account");
        assert_eq!(r.user_id, "svc_1");
        assert_eq!(r.requests, 2);
        assert_eq!(r.input_tokens, 300);
        assert_eq!(r.output_tokens, 50);
        assert_eq!(r.cache_read_tokens, 12);
        assert_eq!(r.cost_micro_cents, 3234);

        let member = rows.iter().find(|r| r.day == "2026-10-01").unwrap();
        assert_eq!(member.user_type, "member");
        assert_eq!(member.user_id, "u1");
        assert_eq!(member.cost_micro_cents, 50);
    }

    /// 认不出的表头 / 空内容只返回空，不 panic。
    #[test]
    fn parse_v1_is_tolerant_of_unknown_headers() {
        assert!(parse_usage_v1_csv("").is_empty());
        assert!(parse_usage_v1_csv("foo,bar\n1,2\n").is_empty());
    }

    /// 真机验证：需要 `OPENCODE_API_KEY=<all 权限 Key> cargo test -- --ignored live_`。
    /// 默认 ignore，避免 CI/离线环境碰网络。
    #[tokio::test]
    #[ignore]
    async fn live_go_status_summary_and_export() {
        let key = std::env::var("OPENCODE_API_KEY").expect("需设置 OPENCODE_API_KEY");
        let c = Client::with_base(key, DEFAULT_BASE_URL);

        let q = c.go_status().await.expect("go/status 应成功");
        assert!(q.month.limit_micro_cents > 0, "月额度应大于 0");
        assert!(q.five_hour.limit_micro_cents > 0);
        assert!(q.ends_at.is_some(), "应有到期时间");

        let s = c.summary("7d").await.expect("summary 应成功");
        assert!(s.total_requests >= 0);

        let hourly = c.cost_by_day("24h", "hour").await.expect("cost-by-day 应成功");
        let _ = hourly;

        let rows = c.export_usage_v2("7d").await.expect("v2 导出应成功");
        assert!(!rows.is_empty(), "近 7 天应有用量记录");
        assert!(rows.iter().all(|r| !r.model.is_empty()));
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

    /// 归到 UTC 日期
    pub fn day(&self) -> String {
        if self.started_at_ms <= 0 {
            return String::new();
        }
        chrono::DateTime::from_timestamp_millis(self.started_at_ms)
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_default()
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

    /// 会话直接查额度（权限足够，无需 Service Key）。
    pub async fn go_status(&self) -> Result<Quota, String> {
        let v = self.get_json("/go/status").await?;
        Ok(Quota::from_json(&v))
    }

    /// 会话查用量汇总
    pub async fn summary(&self, range: &str) -> Result<Summary, String> {
        let v = self.get_json(&format!("/usage/summary?range={range}")).await?;
        Ok(Summary::from_json(&v))
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
