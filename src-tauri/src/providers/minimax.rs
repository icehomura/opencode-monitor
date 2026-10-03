//! MiniMax 平台占位实现。
//!
//! 目前 **尚未** 拿到 MiniMax 的接口文档：登录方式（Cookie / Token / OAuth）、
//! 额度查询接口、逐条请求日志接口、鉴权字段与返回结构全部待定。
//!
//! 因此这里只满足 [`UsageProvider`] 契约、让 `ProviderId::Minimax` 能被正常构造与
//! 枚举；任何真实的网络请求都不会发生——`quota` / `request_logs` 直接返回
//! “尚未接入”错误，`login_url` 留空。
//!
//! 接口文档到位后，按下面的步骤补实现即可（不需要改动调用方）：
//! 1. [`MiniMaxProvider::login_url`] 填真实登录页 URL；
//! 2. [`MiniMaxProvider::quota`] 用文档中的鉴权方式请求额度接口，解析为
//!    [`Quota`]；
//! 3. [`MiniMaxProvider::request_logs`] 用文档中的分页/时间窗口参数请求日志，
//!    解析为 [`RequestLogPage`]；
//! 4. [`MiniMaxProvider::probe`] 登录后探测身份（可读名称 + 平台标识）。
//!
//! 严禁在此臆造 URL、字段名或鉴权方式。

use crate::providers::opencode::{CostPoint, Quota, RequestLogPage};
use crate::providers::{Credentials, Identity, ProviderId, UsageProvider};

/// MiniMax 平台实现（占位）。
///
/// 无内部状态：凭据由 [`Credentials`] 在每次调用时传入。
pub struct MiniMaxProvider;

/// 接口文档未到，所有需要真实后端的能力统一返回该错误。
const NOT_IMPLEMENTED: &str = "MiniMax 支持尚未接入（等待接口文档）";

impl UsageProvider for MiniMaxProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Minimax
    }

    /// 登录页 URL 待接口文档确认，先留空。
    fn login_url(&self) -> &'static str {
        ""
    }

    async fn quota(&self, _creds: &Credentials) -> Result<Quota, String> {
        Err(NOT_IMPLEMENTED.to_string())
    }

    async fn request_logs(
        &self,
        _creds: &Credentials,
        _since_ms: i64,
        _until_ms: i64,
        _cursor: Option<&str>,
        _limit: u32,
    ) -> Result<RequestLogPage, String> {
        Err(NOT_IMPLEMENTED.to_string())
    }

    async fn probe(&self, _creds: &Credentials) -> Identity {
        Identity::default()
    }

    async fn cost_by_day(
        &self,
        _creds: &Credentials,
        _range: &str,
        _bucket: &str,
    ) -> Result<Vec<CostPoint>, String> {
        Err(NOT_IMPLEMENTED.to_string())
    }
}
