# AGENTS.md

## Project Overview

OpenCode Monitor 是一个基于 Tauri 2 的桌面应用，用于监控 **OpenCode Go / Go Plus** 订阅的额度与用量：
从 OpenCode 控制台 API 读取实时额度，并把逐条请求日志同步到本地 SQLite（按天聚合）。
前端 Vue 3 + Vite，后端 Rust + SQLite (WAL)。

## Tech Stack

- **Runtime**: Tauri 2 (Rust backend + WebView frontend)
- **Backend**: Rust, tokio, reqwest (rustls-tls), rusqlite (bundled)
- **Frontend**: Vue 3 (Composition API, `<script setup>`), Vite, ECharts
- **Package Manager**: bun (前端), cargo (后端)
- **IPC**: Tauri invoke / events (Rust <-> Vue)

## 鉴权（重要）

**只用登录会话（WebView 会话 Cookie）；程序不再使用 Service API Key。**

| 能力 | 登录会话（WebView） |
|---|---|
| 额度 `/go/status` | ✅ |
| 用量汇总 | ✅ |
| 逐条日志 `/request-logs` | ✅ |

- 额度、汇总、逐条日志全部依赖登录后的会话 Cookie；未登录时无数据。
- 登录由 `open_login_window` 打开独立 WebView，登录后 `capture_login` 读取 Cookie 并保存；
  `logout` 会删除 WebView 的 Cookie（逐个 `delete_cookie` + 清浏览数据）。
- 不读取 WebView 的 httpOnly Cookie 时，Windows 上必须在 **async 命令**里读（同步会死锁）。

## Project Structure

```
opencode-monitor/
├── src/
│   ├── App.vue               # 根组件（TitleBar / Toolbar / StatsCards / UsageChart）
│   ├── components/
│   │   ├── TitleBar.vue      # 计划 / 到期 / 预估可用时长
│   │   ├── Toolbar.vue       # 同步状态 + 时间范围
│   │   ├── StatsCards.vue    # 第一行词元统计；第二行额度 + 当前模型请求限制
│   │   ├── UsageChart.vue    # 请求次数(正常/异常) + 输出/输入/缓存
│   │   ├── SettingsModal.vue # 账户 / 模型 / 日志 / 系统
│   │   └── base/             # 基础控件
│   ├── composables/useMonitor.js  # 额度 + 序列 + 同步状态 + 模型列表
│   └── utils/format.js       # microCents / 时间 / 词元格式化
├── src-tauri/src/
│   ├── main.rs               # 配置读写、命令注册、登录 WebView、托盘、关闭行为
│   ├── opencode.rs           # API 客户端：SessionClient(Cookie) + /request-logs 分页
│   ├── models.rs / models.json   # 官方文档的每模型额度表（内置）
│   ├── store.rs              # SQLite：usage_daily / request_log / meta
│   └── sync.rs               # 逐条日志同步 + 额度刷新 + 后台循环
└── docs/opencode-api-research.md
```

## 后端要点

### API 客户端 (`opencode.rs`)
- `SessionClient`：`Cookie`（登录后完整 Cookie 头）+ `x-org-id`，浏览器 UA/Referer；
  `request_logs_page(since, until, cursor, limit)` 走 `/request-logs`（cursor 分页，limit ≤ 100）。
- 401/403 有明确中文提示。

### 存储 (`store.rs`)
- `usage_daily` 主键 `(day,user_type,user_id,provider,model)`（逐条日志按天聚合结果）
- `request_log` 主键 `id`（逐条日志：tokens / status / started_at_ms）
- `meta`：`last_sync_ms` / `last_full_sync_ms` / `last_requestlog_ms` / `initialized`

### 同步 (`sync.rs`)
- `sync_request_logs` 拉 `/request-logs` 并按天聚合：全量窗口 30d、增量按 `last_requestlog_ms`。
- 后台循环默认每 5 秒；同时刷新额度，emit `quota-updated` / `sync-status`。

### 配置
- `opencode-monitor.json`：`session_cookie` / `org_id` / `base_url` / `incremental_secs` / `close_action`。
- 读写走 `config_path()` + `read_config_value()` / `update_config_value()` 唯一入口。

## 命令列表

`get_settings`、`save_settings`、`get_quota`、`get_sync_status`、`sync_request_logs_now`、
`get_dashboard`、`get_request_logs`、`get_models`、`get_rpm`、
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
