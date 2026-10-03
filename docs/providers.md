# 平台（Provider）契约

> Usage Monitor 的数据来源统一抽象为**平台**：每个平台一份实现，界面与同步流程对平台无感知。
> 契约定义在 `src-tauri/src/providers/mod.rs`；跨平台共用的数据类型定义在 `src-tauri/src/providers/opencode.rs`。

## 一览

| 平台 | `ProviderId` | 实现文件 | 状态 | 鉴权 | 额度 | 逐条日志 |
|---|---|---|---|---|---|---|
| OpenCode | `opencode` | `providers/opencode.rs` | ✅ 已实现 | 控制台登录会话 Cookie + 工作区 id | `GET /console/api/go/status` | `GET /console/api/request-logs`（cursor 分页，limit ≤ 100） |
| MiniMax | `minimax` | `providers/minimax.rs` | 🚧 占位，待接入 | 待定 | 待定 | 待定 |

OpenCode 平台的接口细节见 [`opencode-api-research.md`](opencode-api-research.md)。

## 类型

### `ProviderId`

平台标识，`serde` 序列化为小写字符串。

```rust
pub enum ProviderId { Opencode, Minimax }   // 序列化为 "opencode" / "minimax"，Default = Opencode
```

- `as_str()` → `"opencode"` / `"minimax"`
- `label()` → `"OpenCode"` / `"MiniMax"`（界面展示名）
- `implemented()` → OpenCode 为 `true`，MiniMax 为 `false`
- `parse(s)` → 大小写不敏感、去空白；不认识返回 `None`
- `all()` → `[Opencode, Minimax]`

### `Credentials`

平台无关的凭据；调用时逐次传入，provider 自身不持有状态。

```rust
pub struct Credentials { pub cookie: String, pub org_id: String }
```

- OpenCode：`cookie` = WebView 登录后的完整 Cookie 头，`org_id` = 工作区 id（`org_...`）。
- 落盘为 `usage-monitor.json` 的 `accounts[]` 中的 `provider` / `cookie` / `org_id` 字段。

### `UsageProvider`

平台实现契约：

```rust
pub trait UsageProvider {
    fn id(&self) -> ProviderId;
    fn login_url(&self) -> &'static str;
    async fn quota(&self, creds: &Credentials) -> Result<Quota, String>;
    async fn request_logs(&self, creds: &Credentials, since_ms: i64, until_ms: i64,
                          cursor: Option<&str>, limit: u32) -> Result<RequestLogPage, String>;
    async fn probe_label(&self, creds: &Credentials) -> Option<String>;
}
```

- `quota`：当前订阅额度（5 小时 / 周 / 月 + 到期信息）。
- `request_logs`：按时间窗口 + cursor 分页拉逐条日志，`limit ≤ 100`。
- `probe_label`：登录后取账号 / 工作区的可读名称；拿不到返回 `None`。
- 错误一律用 `String`（面向用户的中文提示，如 401/403、未接入）。

### `AnyProvider`

`async fn in trait` 不能 `dyn`，因此用静态分发枚举包装：

```rust
pub enum AnyProvider { Opencode(OpenCodeProvider), Minimax(MiniMaxProvider) }
```

`AnyProvider::for_id(id)` 构造，并按同一组方法名转发 `id` / `login_url` / `quota` / `request_logs` / `probe_label`。

## 新增一个平台

接口文档到位后：

1. `providers/mod.rs` 的 `ProviderId` 加变体，并同步 `as_str` / `label` / `implemented` / `parse` / `all`。
2. 新建 `providers/<platform>.rs`，为结构体实现 `UsageProvider`：填 `login_url`，用文档里的鉴权方式实现 `quota` 与 `request_logs`，`probe_label` 取可读名称。
3. `providers/mod.rs` 的 `AnyProvider` 加变体与 `for_id` / 各转发分支。
4. 调用方（`sync.rs`、`main.rs` 的命令）统一走 `AnyProvider::for_id(account.provider)`，不需要为平台写分支。
5. 账号侧只需 `provider` 字段落盘，登录窗口按 `login_url()` 打开、用 `probe_label()` 命名账号。

## MiniMax 待办

目前只拿到占位实现，以下全部待定，**不要臆造 URL / 字段名 / 鉴权方式**：

- 登录方式（Cookie / Token / OAuth）与登录页 URL；
- 额度查询接口与返回结构（映射到 `Quota`）；
- 逐条请求日志接口：时间窗口、分页 / cursor、每页上限（映射到 `RequestLogPage`）；
- 登录后可读名称的获取方式（`probe_label`）。
