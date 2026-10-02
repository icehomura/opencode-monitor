# AGENTS.md

## Project Overview

OpenCode Monitor 是一个基于 Tauri 2 的桌面应用，用于监控 **OpenCode Go / Go Plus** 订阅的额度与用量：
从 OpenCode 控制台 API 读取实时额度，并把云端用量日志同步到本地 SQLite。
前端 Vue 3 + Vite，后端 Rust + SQLite (WAL)。

## Tech Stack

- **Runtime**: Tauri 2 (Rust backend + WebView frontend)
- **Backend**: Rust, tokio, reqwest (rustls-tls), rusqlite (bundled)
- **Frontend**: Vue 3 (Composition API, `<script setup>`), Vite, ECharts
- **Package Manager**: bun (前端), cargo (后端)
- **IPC**: Tauri invoke / events (Rust <-> Vue)

## Project Structure

```
opencode-monitor/
├── src/                      # Vue 前端源码
│   ├── App.vue               # 根组件（TitleBar / Toolbar / StatsCards / UsageChart）
│   ├── components/
│   │   ├── StatsCards.vue    # 5h/周/月额度 + 订阅到期
│   │   ├── UsageChart.vue    # 花费 / 词元曲线
│   │   ├── SettingsModal.vue # 账户 / 日志 / 系统 三个 Tab
│   │   ├── TitleBar.vue, Toolbar.vue
│   │   └── base/             # 基础控件
│   ├── composables/
│   │   ├── useMonitor.js     # 额度 + 汇总 + 同步状态数据源
│   │   ├── useTauri.js, useTheme.js
│   └── utils/format.js       # microCents / 时间 / 词元格式化
├── src-tauri/src/
│   ├── main.rs               # 配置读写、Tauri 命令注册、托盘、关闭行为
│   ├── opencode.rs           # 控制台 API 客户端 + CSV 解析
│   ├── store.rs              # SQLite（usage_daily / meta）
│   └── sync.rs               # 全量/增量同步引擎 + 额度缓存 + 后台循环
└── docs/opencode-api-research.md   # API 调研与实测结果
```

## 后端要点

### API 客户端 (`opencode.rs`)
- 基址默认 `https://opencode.ai/console/api`，鉴权 `Authorization: Bearer <oc_sk_...>`。
- `go_status()` → 额度；`summary()` / `cost_by_day()`；`export_usage_v2(range)` → 解析 CSV。
- 401/403 有明确中文提示（Key 无效 / 权限不足）。

### 存储 (`store.rs`)
- 表 `usage_daily` 主键 `(day, user_type, user_id, provider, model)`，upsert 幂等。
- `meta` 表存 `last_sync_ms` / `last_full_sync_ms` / `initialized`。

### 同步 (`sync.rs`)
- 全量：`range=90d`（接口上限）；增量：`range=7d`，默认每 5 秒。
- 用量数据是**按天汇总**（v2 导出），不是逐条请求日志。
- 后台循环启动时若从未全量则先全量，否则增量；同时刷新额度并 emit
  `quota-updated` / `sync-status` 事件。

### 配置
- `opencode-monitor.json`：`api_key` / `base_url` / `incremental_secs` / `close_action`。
- 读写走 `config_path()` + `read_config_value()` / `update_config_value()` 唯一入口。

## 命令列表

`get_account`、`save_account`（先验证再落盘）、`clear_account`、`get_quota`、
`get_sync_status`、`sync_now`、`sync_full_now`、`get_dashboard`、`get_usage_rows`、
`get_close_action`、`set_close_action`、`get_autostart`、`set_autostart`。

## Build & Run

```bash
bun install
bun run tauri dev       # 前端 + 后端
bun run tauri build     # 打包
cd src-tauri && cargo test
```

## 测试

- `cargo test`：CSV 解析、microCents 解析、SQLite upsert 幂等。
- 真机验证（需要 Key）：`OPENCODE_API_KEY=oc_sk_... cargo test -- --ignored live_`
