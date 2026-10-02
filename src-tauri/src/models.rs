//! 官方文档的每模型额度表（内置）。
//!
//! OpenCode 没有提供「每模型额度」的接口，只有文档表格：
//! - 每模型月美元上限（Go / Go Plus）
//! - 每模型估算请求数（5h / 周 / 月）
//! - 估算用的「每次请求 token 数」（input / cached / output）
//!
//! 数据落在 `models.json`，随二进制编译进来（`include_str!`）。
//! `usd < 0` 表示 Unlimited。

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanLimit {
    /// 月美元上限；-1 = 不限
    pub usd: i64,
    /// 估算请求数；-1 = 不限
    pub req_5h: i64,
    pub req_week: i64,
    pub req_month: i64,
}

impl PlanLimit {
    pub fn unlimited(&self) -> bool {
        self.usd < 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokensPerRequest {
    pub input: i64,
    pub cached: i64,
    pub output: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelLimit {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub tokens_per_request: Option<TokensPerRequest>,
    pub go: PlanLimit,
    pub go_plus: PlanLimit,
}

impl ModelLimit {
    pub fn plan(&self, product: &str) -> &PlanLimit {
        if product == "go-plus" {
            &self.go_plus
        } else {
            &self.go
        }
    }
}

#[derive(Deserialize)]
struct File {
    models: Vec<ModelLimit>,
}

static LIMITS: OnceLock<Vec<ModelLimit>> = OnceLock::new();

pub fn all() -> &'static [ModelLimit] {
    LIMITS.get_or_init(|| {
        serde_json::from_str::<File>(include_str!("models.json"))
            .map(|f| f.models)
            .unwrap_or_default()
    })
}

fn normalize(s: &str) -> String {
    s.trim().to_lowercase().replace('_', "-")
}

pub fn lookup(id: &str) -> Option<&'static ModelLimit> {
    let key = normalize(id);
    all().iter().find(|m| normalize(&m.id) == key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_loads_and_lookup_works() {
        assert!(all().len() >= 25, "内置模型表应已加载");
        let m = lookup("deepseek-v4.1-flash").expect("应能找到 deepseek");
        assert_eq!(m.go.usd, 60);
        assert_eq!(m.go_plus.usd, 120);
        assert_eq!(m.go.req_month, 130000);
        assert!(lookup("DeepSeek_V4.1_Flash").is_some(), "应兼容下划线/大小写");
        assert!(m.tokens_per_request.is_some());
    }
}
