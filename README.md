<p align="center">
  <img src="icons/icon.png" alt="OpenCode Monitor" width="128">
</p>

<h1 align="center">OpenCode Monitor</h1>

<p align="center">
  <strong>OpenCode Go / Go Plus 额度与用量监控</strong>：实时展示 5 小时 / 周 / 月额度，并把云端用量与逐条请求日志同步到本地 SQLite。
</p>

## 功能

- **实时额度面板** — 5 小时、本周、本月三个计量的已用 / 上限 / 剩余 / 重置时间，以及订阅到期日期。
  - 数据来自官方 `GET /console/api/go/status`，无需本地累计。
- **订阅摘要（标题栏）** — 计划名、到期日期、是否到期后不再续费，以及**预估可用时长**（按当前窗口平均消耗速率，结合 5 小时 / 周 / 月额度估算）。
- **仪表盘**
  - 第一行：当前分钟 TPM、词元数明细（输入 / 输出 / 缓存 / 合计）、时间窗口内（总请求数 / 总输出词元数 / 空闲时间 / 平均 RPM / 平均输出词元数/分 / 时间利用率）。
  - 第二行：5 小时 / 本周 / 本月额度卡，以及**当前模型请求限制**（下拉选择用过的模型，三条进度条对比官方估算上限）。
- **图表** — 左轴「请求次数」（正常请求灰 + 异常请求深红堆叠），右轴「输出 / 输入 / 缓存」三条词元曲线（带面积渐变）。时间范围：近1小时 / 当前5小时 / 今日 / 本周 / 本月 / 全部。
- **日志同步（默认每 5 秒）**
  - **登录授权**：走控制台 `/request-logs` 逐条日志，**精确到秒**，数据最新。
  - **API Key 兜底**：走 `/v2/usage/export`（按天汇总，≤90 天）与 `/v1/usage/export`（逐条，≤30 天，仅未迁移 v2 的组织可用），两者合并。
  - 生成器按主键 upsert，云端=本地；设置里可「立即同步 / 全量同步」。

> **为什么详细日志需要登录？** Service API Key **无权**访问 `/request-logs`（返回 403）。逐条明细、异常请求、RPM 只能通过控制台网页登录后的会话获取。API Key 只能拿额度与导出汇总。

## 快速开始

打开应用 → **设置 → 账户**，下面两种方式**任一**即可（都会**自动保存**，没有「保存」按钮）：

1. **API Key（推荐常备）**
   - 到 [OpenCode Console](https://opencode.ai/console) → 工作区 **Service Accounts** 创建一个 Key。
   - **必须选「全部（all）」权限**；「仅推理（inference-only）」读不到额度与用量。
   - 在「API Key」输入框粘贴，失焦即自动验证并保存。
2. **登录授权（拿到逐条日志所需）**
   - 点「登录 OpenCode」→ 弹出的窗口里登录 → 检测到会话后**自动关闭**并保存。
   - 退出登录会同时清空 WebView 的会话 Cookie。

接口基址与同步间隔同样**改动即自动保存**。API Key 与登录会话可同时保留：优先用会话拿逐条日志，失效时自动回退 API Key。

## 配置

配置文件 `opencode-monitor.json`（设置界面会自动写入；含敏感信息，已在 `.gitignore` 中忽略）：

| 键 | 说明 | 默认 |
|---|---|---|
| `api_key` | Service API Key（`oc_sk_...`） | 空 |
| `session_cookie` | 登录会话 Cookie（登录后自动写入） | 空 |
| `org_id` | 工作区 id（登录后自动写入） | 空 |
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

- `/request-logs` 仅登录会话可用（API Key 403）；未登录时只有按天/小时汇总。
- v2 用量导出是**按天汇总**（一天 × 用户 × provider × model 一行），`range` 上限 90 天，因此「全量」= 最近 90 天。
- v1 逐条日志只有**未迁移到 v2** 的组织可用；已迁移的组织调用即 403。
- 会话 Cookie 会过期，过期后需重新登录；API Key 默认长期有效。
- 仅支持 OpenCode Go / Go Plus 订阅；其他计划 `go/status` 可能不可用。
