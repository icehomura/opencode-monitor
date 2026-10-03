//! 平台抽象层：把「用量数据来源」收敛到一个接口。
//!
//! 每个平台是一个 [`UsageProvider`] 实现：
//! * [`opencode::OpenCodeProvider`]：控制台登录会话（Cookie + 工作区 id）；
//! * [`minimax::MiniMaxProvider`]：占位，接口文档到位后再补。
//!
//! 因为 trait 里用了 `async fn`（不能 `dyn`），调用侧统一走静态分发的
//! [`AnyProvider`] 包装：按 [`ProviderId`] 选实现，再委托调用。
//! 凭据是平台无关的 [`Credentials`]，由调用方每次传入，provider 自身不持状态。

pub mod minimax;
pub mod opencode;

pub use minimax::MiniMaxProvider;
pub use opencode::OpenCodeProvider;

use opencode::{CostPoint, Quota, RequestLogPage};

/// 平台标识。持久化/前端传输统一用 `as_str()` 的小写形式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderId {
    Opencode,
    Minimax,
}

impl Default for ProviderId {
    fn default() -> Self {
        ProviderId::Opencode
    }
}

impl ProviderId {
    /// 持久化/传输用的稳定小写标识。
    pub fn as_str(self) -> &'static str {
        match self {
            ProviderId::Opencode => "opencode",
            ProviderId::Minimax => "minimax",
        }
    }

    /// 界面上展示的名字。
    pub fn label(self) -> &'static str {
        match self {
            ProviderId::Opencode => "OpenCode",
            ProviderId::Minimax => "MiniMax",
        }
    }

    /// 是否已有真实实现（未实现的平台只用于占位/展示）。
    pub fn implemented(self) -> bool {
        match self {
            ProviderId::Opencode => true,
            ProviderId::Minimax => false,
        }
    }

    /// 解析平台标识：大小写不敏感，忽略首尾空白；未知值返回 `None`。
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "opencode" => Some(ProviderId::Opencode),
            "minimax" => Some(ProviderId::Minimax),
            _ => None,
        }
    }

    /// 枚举顺序即界面展示顺序。
    pub fn all() -> [Self; 2] {
        [ProviderId::Opencode, ProviderId::Minimax]
    }
}

/// 平台无关的登录凭据。
///
/// OpenCode 用「会话 Cookie + 工作区 id」；别的平台将来按需复用这两个字段
/// （`org_id` 可放工作区/租户标识），不为单一平台加特例字段。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Credentials {
    pub cookie: String,
    pub org_id: String,
}

/// 登录后探测到的身份信息：可读名称 + 平台标识（OpenCode 即工作区 id）。
#[derive(Debug, Clone, Default)]
pub struct Identity {
    pub label: Option<String>,
    pub org_id: Option<String>,
}

/// 一个平台实现要提供的能力：登录页、实时额度、逐条日志、登录后探测身份。
#[allow(async_fn_in_trait)]
pub trait UsageProvider {
    /// 本实现对应的平台。
    fn id(&self) -> ProviderId;

    /// 用户在该平台登录用的页面地址。
    fn login_url(&self) -> &'static str;

    /// 实时额度（5 小时 / 周 / 月计）。
    async fn quota(&self, creds: &Credentials) -> Result<Quota, String>;

    /// 按时间窗 + 游标分页拉取逐条请求日志。
    async fn request_logs(
        &self,
        creds: &Credentials,
        since_ms: i64,
        until_ms: i64,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<RequestLogPage, String>;

    /// 凭据可用时探测身份信息（可读名称 + 平台标识）；探测不到返回 `Identity::default()`。
    async fn probe(&self, creds: &Credentials) -> Identity;

    /// 云端成本曲线（本地无数据时的兜底）：`bucket` 为 `hour` / `day`。
    async fn cost_by_day(
        &self,
        creds: &Credentials,
        range: &str,
        bucket: &str,
    ) -> Result<Vec<CostPoint>, String>;
}

/// 平台实现的具体载体（`async fn in trait` 不能 `dyn`，故用 enum + match 静态分发）。
pub enum AnyProvider {
    Opencode(OpenCodeProvider),
    Minimax(MiniMaxProvider),
}

impl AnyProvider {
    /// 按平台标识构造对应实现。
    pub fn for_id(id: ProviderId) -> Self {
        match id {
            ProviderId::Opencode => AnyProvider::Opencode(OpenCodeProvider),
            ProviderId::Minimax => AnyProvider::Minimax(MiniMaxProvider),
        }
    }

    /// 当前包装的平台。
    pub fn id(&self) -> ProviderId {
        match self {
            AnyProvider::Opencode(p) => p.id(),
            AnyProvider::Minimax(p) => p.id(),
        }
    }

    /// 登录页地址，直接委托给具体实现。
    pub fn login_url(&self) -> &'static str {
        match self {
            AnyProvider::Opencode(p) => p.login_url(),
            AnyProvider::Minimax(p) => p.login_url(),
        }
    }

    /// 实时额度，直接委托给具体实现。
    pub async fn quota(&self, creds: &Credentials) -> Result<Quota, String> {
        match self {
            AnyProvider::Opencode(p) => p.quota(creds).await,
            AnyProvider::Minimax(p) => p.quota(creds).await,
        }
    }

    /// 逐条日志，直接委托给具体实现。
    pub async fn request_logs(
        &self,
        creds: &Credentials,
        since_ms: i64,
        until_ms: i64,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<RequestLogPage, String> {
        match self {
            AnyProvider::Opencode(p) => p.request_logs(creds, since_ms, until_ms, cursor, limit).await,
            AnyProvider::Minimax(p) => p.request_logs(creds, since_ms, until_ms, cursor, limit).await,
        }
    }

    /// 探测身份信息，直接委托给具体实现。
    pub async fn probe(&self, creds: &Credentials) -> Identity {
        match self {
            AnyProvider::Opencode(p) => p.probe(creds).await,
            AnyProvider::Minimax(p) => p.probe(creds).await,
        }
    }

    /// 云端成本曲线，直接委托给具体实现。
    pub async fn cost_by_day(
        &self,
        creds: &Credentials,
        range: &str,
        bucket: &str,
    ) -> Result<Vec<CostPoint>, String> {
        match self {
            AnyProvider::Opencode(p) => p.cost_by_day(creds, range, bucket).await,
            AnyProvider::Minimax(p) => p.cost_by_day(creds, range, bucket).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ignores_case_and_surrounding_whitespace() {
        assert_eq!(ProviderId::parse("opencode"), Some(ProviderId::Opencode));
        assert_eq!(ProviderId::parse("OpenCode"), Some(ProviderId::Opencode));
        assert_eq!(ProviderId::parse("OPENCODE"), Some(ProviderId::Opencode));
        assert_eq!(ProviderId::parse("  OpenCode\t"), Some(ProviderId::Opencode));
        assert_eq!(ProviderId::parse("\nminimax "), Some(ProviderId::Minimax));
        assert_eq!(ProviderId::parse("MiniMax"), Some(ProviderId::Minimax));
    }

    #[test]
    fn parse_rejects_unknown_and_empty() {
        assert_eq!(ProviderId::parse(""), None);
        assert_eq!(ProviderId::parse("   "), None);
        assert_eq!(ProviderId::parse("openai"), None);
        assert_eq!(ProviderId::parse("opencode2"), None);
        assert_eq!(ProviderId::parse("open code"), None);
    }

    #[test]
    fn all_returns_opencode_first_and_implemented_flags_match() {
        assert_eq!(ProviderId::all(), [ProviderId::Opencode, ProviderId::Minimax]);
        assert!(ProviderId::Opencode.implemented());
        assert!(!ProviderId::Minimax.implemented());
    }

    #[test]
    fn default_is_opencode() {
        assert_eq!(ProviderId::default(), ProviderId::Opencode);
    }

    #[test]
    fn as_str_label_and_parse_round_trip() {
        for id in ProviderId::all() {
            assert_eq!(ProviderId::parse(id.as_str()), Some(id));
        }
        assert_eq!(ProviderId::Opencode.as_str(), "opencode");
        assert_eq!(ProviderId::Minimax.as_str(), "minimax");
        assert_eq!(ProviderId::Opencode.label(), "OpenCode");
        assert_eq!(ProviderId::Minimax.label(), "MiniMax");
    }

    #[test]
    fn provider_id_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&ProviderId::Opencode).unwrap(),
            "\"opencode\""
        );
        assert_eq!(
            serde_json::to_string(&ProviderId::Minimax).unwrap(),
            "\"minimax\""
        );
        assert_eq!(
            serde_json::from_str::<ProviderId>("\"minimax\"").unwrap(),
            ProviderId::Minimax
        );
    }
}
