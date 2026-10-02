# AGENTS.md

## Project Overview

OpenCode Monitor 是一个基于 Tauri 2 的桌面应用，用于监控 **OpenCode Go / Go Plus** 订阅的额度与用量：
从 OpenCode 控制台 API 读取实时额度，并把逐条请求日志同步到本地 SQLite（按天聚合）。
前端 Vue 3 + Vite，后端 Rust + SQLite (WAL)。

## 项目声明（给协作者 / Agent）

定位：**只读的本地展示工具** —— 把本人账号的额度与用量元数据同步到本地并可视化。

- 不要新增：绕过额度 / 限流 / 计费 / 风控的能力；自动切换或轮换账号；代理转发（AI 网关）；批量注册或代管他人账号；抓取或存储提示词与模型输出；任何上报 / 遥测。
- 多账号 = 多个本人账号各自同步、各自展示（数据按账号隔离），不存在账号间调度或轮换。
- 面向用户的告知只写一遍：README「声明」+ 首次启动的「使用须知」弹窗。不要在代码注释里堆免责话术。

## Tech Stack

- **Runtime**: Tauri 2 (Rust backend + WebView frontend)
- **Backend**: Rust, tokio, reqwest (rustls-tls), rusqlite (bundled)
- **Frontend**: Vue 3 (Composition API, `<script setup>`), Vite, ECharts
- **Package Manager**: bun (前端), cargo (后端)
- **IPC**: Tauri invoke / events (Rust <-> Vue)

## 鉴权与多账号（重要）

**只用登录会话（WebView 会话 Cookie）；程序不使用 Service API Key。**

| 能力 | 登录会话（WebView） |
|---|---|
| 额度 `/go/status` | ✅ |
| 用量汇总 | ✅ |
| 逐条日志 `/request-logs` | ✅ |

- 额度、汇总、逐条日志全部依赖登录后的会话 Cookie；未登录时无数据。
- 支持**多账号**：每个账号一份会话 Cookie，**数据按 `account_id` 隔离**，后台循环逐个账号同步。
- **主账号**（`primary_account`）：标题栏（计划 / 到期 / 预估可用时长）与第二行的额度卡显示主账号数据；「当前模型请求限制」卡片自带账号下拉（`get_models({ accountId })`：省略 = 主账号，空串 = 全部账号，否则指定账号），默认「全部账号」。图表与第一行卡片是所有账号的聚合。
- 添加账号 = 新开 WebView 登录（`open_login_window({ addNew: true })`）后 `capture_login` 落盘；
  `logout` 删除 WebView Cookie 罐并清掉该账号凭据（账号条目保留，可用 `remove_account` 删除）。
- 不读取 WebView 的 httpOnly Cookie 时，Windows 上必须在 **async 命令**里读（同步会死锁）。

## Project Structure

```
opencode-monitor/
├── src/
│   ├── App.vue               # 根组件（TitleBar / Toolbar / StatsCards / UsageChart）
│   ├── components/
│   │   ├── TitleBar.vue      # 计划 / 到期 / 预估可用时长（主账号）
│   │   ├── Toolbar.vue       # 同步状态 + 时间范围（含自定义区间）
│   │   ├── StatsCards.vue    # 第一行词元统计；第二行额度 + 当前模型请求限制（主账号）
│   │   ├── UsageChart.vue    # 请求次数(正常/异常) + 输出/输入/缓存
│   │   ├── DatePickerModal.vue    # 自定义时间范围选择（日期/周/月份/指定范围）
│   │   ├── SettingsModal.vue # 账户（账号管理）/ 模型 / 日志 / 系统
│   │   ├── AccountEditModal.vue   # 账号重命名 / 设为主账号
│   │   └── base/             # 基础控件
│   ├── composables/useMonitor.js  # 额度 + 序列 + 同步状态 + 模型列表 + 账号列表
│   └── utils/format.js       # microCents / 时间 / 词元格式化
├── src-tauri/src/
│   ├── main.rs               # 配置读写、命令注册、登录 WebView、托盘、关闭行为
│   ├── accounts.rs           # 多账号凭据（accounts[] / primary_account / 登录落盘）
│   ├── opencode.rs           # API 客户端：SessionClient(Cookie) + /request-logs 分页
│   ├── models.rs / models.json   # 官方文档的每模型额度表（内置）
│   ├── store.rs              # SQLite：usage_daily / request_log / meta（都带 account_id）
│   └── sync.rs               # 逐账号日志同步 + 额度刷新 + 后台循环
└── docs/opencode-api-research.md
```

## 后端要点

### API 客户端 (`opencode.rs`)
- `SessionClient`：`Cookie`（登录后完整 Cookie 头）+ `x-org-id`，浏览器 UA/Referer；
  `request_logs_page(since, until, cursor, limit)` 走 `/request-logs`（cursor 分页，limit ≤ 100）。
- 401/403 有明确中文提示。

### 账户 (`accounts.rs`)
- 账号 = `{ id, name, org_id, cookie }`；`id` 优先用工作区 id（`org_...`）。
- 账号列表存在 `opencode-monitor.json` 的 `accounts` 数组；单账号时代的 `session_cookie` / `org_id` **不再读取**（升级后需重新登录一次，不做配置迁移）。

### 存储 (`store.rs`)
- `request_log` 的 `cost_micro_cents`：`/request-logs` 的 `cost` 是**美元浮点**（实测如 `0.00113096`），
  入库前 ×1e8 换算成 microCents（与前端 `fmtUsd` 一致）；按整数解析会全变 0。
- `usage_daily` 主键 `(account_id,day,user_type,user_id,provider,model)`（逐条日志按天聚合结果）
- `request_log` 主键 `(account_id,id)`（逐条日志：tokens / status / started_at_ms）
- `meta`：全局 `last_sync_ms` / `schema_version`；账号维度 `last_sync_ms:<id>` / `last_full_sync_ms:<id>` / `last_requestlog_ms:<id>` / `initialized:<id>`
- 本地库只是云端日志的缓存：`meta.schema_version` 与当前版本不一致时**整库重建**，由重新同步恢复，不做数据迁移。

### 同步 (`sync.rs`)
- `sync_request_logs(account)` 拉 `/request-logs` 并按天聚合：全量窗口 30d、增量按该账号的 `last_requestlog_ms:<id>`。
- 后台循环默认每 30 秒（最小 10 秒，`incremental_secs`），**逐个账号**同步并刷新各自额度，emit `quota-updated` / `sync-status`。
- 频率与退避保持保守（分页之间 250ms、429/5xx 冷却 120s，见 `COOLDOWN_UNTIL`），不要为了「更实时」调激进。

### 配置
- `opencode-monitor.json`：`accounts`（数组）/ `primary_account` / `base_url` / `incremental_secs` / `close_action`。
- 读写走 `config_path()` + `read_config_value()` / `update_config_value()` 唯一入口。

## 命令列表

`get_settings`、`save_settings`、`list_accounts`、`set_primary_account`、`rename_account`、`remove_account`、
`get_quota`（可带 `accountId`）、`get_sync_status`、`sync_request_logs_now`、
`get_dashboard`（可带 `sinceMs` / `untilMs`）、`get_request_logs`、`get_models`（可带 `accountId`）、`get_rpm`、
`open_login_window`、`capture_login`、`login_status`、`logout`、
`get_close_action`、`set_close_action`、`get_autostart`、`set_autostart`。

## Build & Run

```bash
bun install
bun run tauri dev       # 前端 + 后端
bun run tauri build     # 打包
cd src-tauri && cargo test
```

## 测试

- `cargo test`：microCents 解析、SQLite upsert 幂等、逐条日志按天聚合。
