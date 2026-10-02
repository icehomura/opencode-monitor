//! 多账号凭据管理。
//!
//! 账号列表存在 `opencode-monitor.json` 的 `accounts` 数组里，每个账号一份
//! 控制台会话 Cookie；`primary_account` 记录主账号 id。主账号的数据用于标题栏
//! （计划 / 到期 / 预估可用时长）与第二行卡片（额度 / 模型请求限制），
//! 其余账号只参与日志同步与图表聚合。
//!
//! 单账号时代的 `session_cookie` / `org_id` 会在启动时迁移成第一个账号（幂等）。

use crate::opencode::SessionClient;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Account {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub org_id: String,
    #[serde(default)]
    pub cookie: String,
}

impl Account {
    pub fn logged_in(&self) -> bool {
        !self.cookie.trim().is_empty()
    }

    pub fn display_name(&self) -> String {
        if !self.name.trim().is_empty() {
            return self.name.clone();
        }
        if !self.org_id.trim().is_empty() {
            return self.org_id.clone();
        }
        self.id.clone()
    }

    /// 该账号的会话客户端（未登录时报错）。
    pub fn session(&self) -> Result<SessionClient, String> {
        if !self.logged_in() {
            return Err(format!("账号「{}」未登录", self.display_name()));
        }
        let org = self.org_id.trim();
        Ok(SessionClient::new(
            self.cookie.clone(),
            crate::base_url(),
            if org.is_empty() { None } else { Some(org.to_string()) },
        ))
    }
}

/// 账号列表。
pub fn list() -> Vec<Account> {
    let cfg = crate::read_config_value();
    let mut out: Vec<Account> = cfg
        .get("accounts")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    out.retain(|a| !a.id.trim().is_empty());
    out
}

pub fn find(id: &str) -> Option<Account> {
    list().into_iter().find(|a| a.id == id)
}

/// 主账号 id；配置缺失时回退到第一个账号。
pub fn primary_id() -> String {
    let accounts = list();
    let want = crate::read_config_value()
        .get("primary_account")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if accounts.iter().any(|a| a.id == want) {
        return want;
    }
    accounts.first().map(|a| a.id.clone()).unwrap_or_default()
}

pub fn primary() -> Option<Account> {
    let id = primary_id();
    list().into_iter().find(|a| a.id == id)
}

/// 账号 id：优先用工作区 id，没有就按时间生成一个。
pub fn new_id(org_id: &str) -> String {
    let org = org_id.trim();
    if !org.is_empty() {
        return org.to_string();
    }
    format!("acct_{}", chrono::Utc::now().timestamp_millis())
}

/// 写入（或按 id 合并）一个账号。
pub fn upsert(acc: Account) -> Result<(), String> {
    let id = acc.id.clone();
    crate::update_config_value(|v| {
        let mut arr = list();
        match arr.iter_mut().find(|a| a.id == id) {
            Some(slot) => *slot = acc,
            None => arr.push(acc),
        }
        v["accounts"] = json!(arr);
        let has_primary = v
            .get("primary_account")
            .and_then(|p| p.as_str())
            .map(|p| !p.is_empty())
            .unwrap_or(false);
        if !has_primary {
            v["primary_account"] = json!(id);
        }
    })
}

pub fn set_primary(id: &str) -> Result<(), String> {
    if find(id).is_none() {
        return Err("账号不存在".into());
    }
    crate::update_config_value(|v| v["primary_account"] = json!(id))
}

pub fn rename(id: &str, name: &str) -> Result<(), String> {
    let name = name.trim().to_string();
    crate::update_config_value(|v| {
        let mut arr = list();
        if let Some(a) = arr.iter_mut().find(|a| a.id == id) {
            a.name = name;
        }
        v["accounts"] = json!(arr);
    })
}

/// 退出登录：清掉凭据但保留账号条目。
pub fn clear_cookie(id: &str) -> Result<(), String> {
    crate::update_config_value(|v| {
        let mut arr = list();
        if let Some(a) = arr.iter_mut().find(|a| a.id == id) {
            a.cookie = String::new();
        }
        v["accounts"] = json!(arr);
    })
}

/// 删除账号（凭据 + 条目）；本地数据是否清理由调用方决定。
pub fn remove(id: &str) -> Result<(), String> {
    crate::update_config_value(|v| {
        let mut arr = list();
        arr.retain(|a| a.id != id);
        v["accounts"] = json!(arr);
        let primary = v.get("primary_account").and_then(|p| p.as_str()).unwrap_or("");
        if primary == id {
            let next = list().first().map(|a| a.id.clone()).unwrap_or_default();
            v["primary_account"] = json!(next);
        }
    })
}
