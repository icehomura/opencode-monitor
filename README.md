<p align="center">
  <img src="icons/icon.png" alt="OpenCode Monitor" width="128">
</p>

<h1 align="center">OpenCode Monitor</h1>

<p align="center">
  <strong>OpenCode Go / Go Plus 额度与用量监控</strong>：实时展示 5 小时 / 周 / 月剩余额度，并把云端用量日志同步到本地 SQLite。
</p>

## 功能

- **实时额度面板** — 5 小时、本周、本月三个计量的已用 / 上限 / 剩余 / 重置时间，以及订阅到期日期。
  - 数据来自官方 `GET /console/api/go/status`，无需本地累计。
- **云端日志同步** — 同时尝试两个官方导出接口并合并，写入本地 SQLite：
  - **v2** `/console/api/v2/usage/export`（按天汇总，最多 **90 天**）
  - **v1** `/console/api/v1/usage/export`（逐条日志，最多 **30 天**，仅未迁移 v2 的组织可用）
  - 任一接口 403/不可用只记录备注、不报错；两个都失败才提示。合并时 v1 覆盖 v2（最近数据更新鲜）。
  - 首次启动自动**全量同步**；之后按设定间隔（默认 **5 秒**）**增量同步**最近 7 天，主键 upsert 保证云端=本地。
  - 设置里可手动「立即同步 / 全量同步」。
- **成本曲线** — 按小时（近 24 小时）或按天（7/30/全部）展示花费与词元用量。
- **日志浏览** — 设置 → 日志，分页查看同步到本地的记录。

## 快速开始

1. 打开 [OpenCode Console](https://opencode.ai/console)，进入工作区 **Service Accounts** 页面创建一个 Key。
   - **必须选择「全部（all）」权限**；「仅推理（inference-only）」的 Key 读不到用量与额度。
2. 打开本应用 → 设置 → 账户，填入 API Key，点「保存并验证」。
3. 验证通过后额度面板立即显示；日志会在后台自动同步。

## 配置

配置文件 `opencode-monitor.json`（也可全部在设置界面修改）：

| 键 | 说明 | 默认 |
|---|---|---|
| `api_key` | Service API Key（`oc_sk_...`） | 空 |
| `base_url` | 控制台 API 基址 | `https://opencode.ai/console/api` |
| `incremental_secs` | 增量同步间隔（秒） | `5` |
| `close_action` | 关闭按钮行为 `ask` / `minimize` / `quit` | `ask` |

**文件位置**：

| 平台 | 位置 |
|---|---|
| Windows | exe 同目录 |
| macOS | `~/Library/Application Support/com.icehomura.opencode-monitor/` |
| Linux AppImage | `.AppImage` 同目录 |

## 构建

```bash
bun install
bun run tauri dev     # 开发
bun run tauri build   # 打包
```

## 接口调研

详见 [`docs/opencode-api-research.md`](docs/opencode-api-research.md)（含实测结果、CSV 表头、额度结构）。

## 已知限制

- v2 用量导出是**按天汇总**（一天 × 用户 × provider × model 一行），`range` 上限 90 天，因此「全量」= 最近 90 天。
- v1 逐条日志只有**未迁移到 v2** 的组织可用；已迁移的组织调用即 403，此时只同步 v2 汇总（数据粒度更粗、最新数据可能滞后）。
- 若某天只有 v2 数据，日志会是当天汇总；若 v1 可用，最近 30 天会被逐条日志聚合后的结果覆盖。
- 仅支持 OpenCode Go / Go Plus 订阅；其他计划 `go/status` 可能不可用。
