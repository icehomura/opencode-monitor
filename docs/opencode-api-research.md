# OpenCode 云端 API 调研（用于 OpenCode Monitor）

> 来源：`https://opencode.ai/console` 前端 bundle（`/console/assets/index-*.js`）中的 Effect HttpApi 定义，
> 以及官方文档 `https://opencode.ai/docs/go/`、`/docs/zen/`。
> 实测时间：2026-10-02。

## 1. 控制台 API 基址

前端资源在 `https://opencode.ai/console/`，所有 API 为**同源相对路径**，即：

```
https://opencode.ai/console/api/...
```

实测（无鉴权）：

| 请求 | 结果 |
|---|---|
| `GET /console/api/usage/summary` | `401 {"_tag":"Unauthorized"}` |
| `GET /console/api/v1/usage/export?scope=organization&range=24h` | `401 {"_tag":"Unauthorized"}` |
| `GET /console/api/billing/account` | `401 {"_tag":"Unauthorized"}` |
| `GET https://opencode.ai/api/...` | 404（Astro 页面，非 API） |
| `GET https://api.opencode.ai/` | `Hello, world!`（另一个 Cloudflare Worker，非控制台 API） |

> 注意：控制台 API **不在** `opencode.ai/api`，也不是 `api.opencode.ai`，而是 `opencode.ai/console/api`。

## 2. Service API Key（工作区里生成的 Key）——历史记录

> **历史记录**：以下为 API 调研事实。本项目**已废除 API Key**，程序只使用控制台登录会话（见第 7 节）。

- Key 前缀：`oc_sk_...`（新）或 `sk-...`（legacy）。（历史）
- 权限枚举：`eh = ["all", "inference-only"]`
  - **全部** = `all` → 可读取用量/账单等
  - **仅推理** = `inference-only` → 只能调用模型，读不了用量
- 生成接口（管理端）：
  - `POST /console/api/service-accounts` `{ name }`
  - `POST /console/api/service-accounts/:serviceAccountId/keys` `{ name, permissions, expiresAt }`
  - 返回 `{ key, token }`；`expiresAt` 即 Key 到期时间。

## 3. 用量 / 日志相关接口

### 3.1 逐条日志导出（CSV）——v1

```
GET /console/api/v1/usage/export
  ?scope=organization|member|service_account|model   (默认 organization)
  &range=24h|7d|30d
  &user_email=...  &service_account_id=...  &provider=...  &model=...
Accept: text/csv
```

- 返回 CSV，**按时间倒序**，流式返回。
- 文档原文：*Stable service-account usage export operations.*
- ⚠️ **仅对尚未迁移到 v2 的组织可用；已迁移的组织返回 403。**

### 3.2 按天汇总导出（CSV）——v2

```
GET /console/api/v2/usage/export?range=7d|30d|90d&user_id=<member|serviceAccountId>
```

- 每天一行（UTC），按 组织成员/service account + provider + model 汇总。
- 数据来自每日 rollup，**可能滞后**；rollup 未就绪时返回 503。
- `range` 覆盖整 UTC 天（含今天）。

### 3.3 仪表盘 JSON 接口

```
GET /console/api/usage/summary      ?range=24h|7d|30d|all &since=<ISO8601> &user_id &service_account_id
GET /console/api/usage/cost-by-day  ?range &since &bucket=day|hour &user_id &service_account_id
GET /console/api/usage/models       ?range &since &page &pageSize(<=100) &costOrder=asc|desc &includeLegacyKeys=true
GET /console/api/usage/users        ?range &since &page &pageSize &costOrder
```

- 有分页：`page`、`pageSize`（1..100）。
- 记录字段（服务端模型）：`provider, model, inputTokens, outputTokens, reasoningTokens,
  cacheReadTokens, cacheWrite5mTokens, cacheWrite1hTokens, costMicroCents, createdAt, ...`

## 4. 额度 / 订阅（Go 计划）

### 4.1 额度结构（前端类型定义）

```ts
fiveHour = { startsAt, resetsAt, limitMicroCents, usedMicroCents }
week     = { startsAt, resetsAt, limitMicroCents, usedMicroCents }
month    = { resetsAt, limitMicroCents, usedMicroCents }
access   = { fiveHour, week, month }
subscription = { startsAt, endsAt, cancelAtPeriodEnd, meters: access }
```

### 4.2 计划常量（前端 bundle 内硬编码）

| 计划 | 价格 | 5 小时 | 周 | 月 |
|---|---|---|---|---|
| Go | $10/月 | $12 | $30 | $60 |
| Go Plus | $40/月 | $48 | $120 | $240 |

> 单位 `microCents`：1 美元 = 1e8 microCents。

### 4.3 获取订阅额度的官方接口

```
GET /console/api/internal/orgs/:orgId/go/status   -> subscription(access.meters)
POST /console/api/internal/orgs/:orgId/go/cancel
POST /console/api/internal/orgs/:orgId/go/restore-paid-access
```

- 这些在 `/api/internal` 下，由内部管理端调用（middleware `Xf`），需要内部/管理鉴权。
- 程序实现：5h/周/月剩余额度直接读 `/go/status` 的 meters；到期时间用 `access.endsAt`。

## 5. 鉴权（已实测）

> 以下为历史实测结论；程序**已废除 API Key**，现在只用控制台登录会话 Cookie（WebView 登录后捕获）。

- **`Authorization: Bearer <oc_sk_...>`** ✅（历史）
- `x-opencode-api-key`、`x-api-key` 均 401；无效 Key 与缺失 Key 都返回 `401 {"_tag":"Unauthorized"}`。
- 会话：控制台同源请求自动带 Cookie；`/request-logs` 仅会话可用（Bearer 403）。

## 6. 实测结果（2026-10-02，一把 `all` 权限 Key；历史记录）

| 接口 | 结果 |
|---|---|
| `GET /go/status` | ✅ 200，返回完整订阅 + 三个 meters |
| `GET /usage/summary` | ✅ 200 |
| `GET /usage/cost-by-day?bucket=hour` | ✅ 200，按小时分桶 |
| `GET /usage/models` | ✅ 200，分页 |
| `GET /usage/users` | ✅ 200，分页 |
| `GET /v2/usage/export?range=30d` | ✅ 200，CSV 按天汇总 |
| `GET /v1/usage/export` | ❌ 403（组织已迁移 v2） |
| `GET /internal/orgs/:orgId/go/status` | ❌ 401 |
| `GET /billing/account` | ❌ 403 |
| `GET /orgs/current` | ❌ 401（仅 session） |

### 6.1 `go/status` 真实响应（节选）

```json
{
  "product": "go",
  "cancelAtPeriodEnd": true,
  "access": {
    "startsAt": "2026-10-02T06:52:39.000Z",
    "endsAt": "2026-11-02T06:52:39.000Z",
    "meters": {
      "fiveHour": { "startsAt": "...", "resetsAt": "...", "limitMicroCents": "1200000000", "usedMicroCents": "50030478" },
      "week":     { "startsAt": "...", "resetsAt": "...", "limitMicroCents": "3000000000", "usedMicroCents": "50030478" },
      "month":    { "resetsAt": "...", "limitMicroCents": "6000000000", "usedMicroCents": "50030478" }
    }
  },
  "upgradePrice": { "amountMicroCents": "3008000000", "currency": "usd" }
}
```

→ **额度直接来自 `/go/status`，无需本地累计**（用户选择的方案 B 成立）。
到期时间 = `access.endsAt`。

### 6.2 v2 CSV 表头（历史：按天汇总导出，程序已不再使用）

```
day,user_type,user_id,user_name,provider,model,requests,
input_tokens,output_tokens,cache_read_tokens,
cache_write_5m_tokens,cache_write_1h_tokens,cost_micro_cents
```

示例行：

```
2026-10-02,service_account,svcacct_01M3...,claude,opencode-go,deepseek-v4.1-flash,231,400725,144689,32290560,0,0,39493787
```

> 程序现在的同步源是会话 Cookie 调用的 `/request-logs` 逐条 JSON（cursor 分页，limit ≤ 100），
> 本地按天聚合；上面的 CSV 导出仅作历史记录保留。

## 7. 结论 / 设计约束

> 历史方案（v2/v1 CSV 导出、按天兜底、API Key 回退）**已废除**：程序不再使用 API Key，
> 逐条数据统一走控制台登录会话。

1. 同步对象是 **`/request-logs` 逐条日志**（精确到秒），落库后本地按天聚合；
   唯一键为 request_log 的 `id`，用 upsert 保证云端=本地。
2. 额度/到期直接读 `/go/status`；图表与 RPM 由本地逐条日志聚合。
3. 增量同步：按 `last_requestlog_ms` 续拉，天然幂等。
4. 全量同步：拉最近 30d 逐条日志并 upsert。
5. 基址默认 `https://opencode.ai/console/api`；所有请求带登录会话 Cookie，未登录时无数据。
