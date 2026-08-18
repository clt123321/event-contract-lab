# SignalX 清洁室源码勘察记录 v0.1

勘察日期：2026-08-17；2026-08-18 补充授权控制台深挖

最新的 Agent、Profile、订单行为和 150 亿行仓库统计见
[`OBSERVABLE_SURFACE_FINDINGS_20260818.md`](./OBSERVABLE_SURFACE_FINDINGS_20260818.md)。

## 结论

当前没有发现 SignalX 的公开源码仓库，也没有发现可用的 JavaScript sourcemap。浏览器会公开接收一个约 998 KB 的主前端包及若干约 290–300 KB 的页面包；这些产物足以恢复控制台的技术栈、路由、API 方法和 WebSocket 事件，但不包含服务端采集器、回测器、交易引擎、数据库实现或完整策略源码。

因此，建议把目标定义为“根据公开行为与接口重新实现等价能力”，而不是从压缩包还原或复制原项目。未调用任何修改账户、下单、启停 Agent、发布配置或 KMS 操作。

## 已观察到的公开前端资产

| 资产 | 大小 | 作用推断 |
|---|---:|---|
| `index-Df5LWhfI.js` | 997,903 B | 主应用、路由、API client、Agent 控制界面 |
| `index-D__79fh_.js` | 299,845 B | 账户管理页面懒加载包 |
| `index-Bck4KEe_.js` | 290,688 B | 数据仓库页面懒加载包 |
| `splitPathsBySizeLoader-DZDwVTok.js` | 281 B | 图表路径分块加载器 |

主包及页面包均无 `sourceMappingURL`。对标准相邻地址 `index-Df5LWhfI.js.map` 的一次 HEAD 检查返回 SPA HTML，而不是 sourcemap。

没有在公开包内发现指向 SignalX 自有 GitHub/GitLab 仓库的 URL。

## 前端技术栈

- Vite 构建的 React 单页应用。
- `react-router` 管理客户端路由。
- TanStack Query 风格的 `QueryClient` 管理查询缓存与失效。
- Blueprint 组件/图标体系。
- Recharts 图表。
- 同源 REST API 与原生 WebSocket。
- Cloudflare CDN/RUM。
- 中英文与 light/dark/system 主题。

## 已恢复的前端路由

```text
/login
/accounts
/accounts/:accountId
/pnl
/trading
/trading/agents
/trading/agents/:agentId
/trading/profiles
/trading/profiles/:profileId
/trading/profiles/new
/trading/:tab
/market
/warehouse
```

## 可见服务边界

```mermaid
flowchart LR
    UI[React Console] --> BFF[/api/* BFF]
    UI --> AGENT[/agent/api/* Agent Control]
    UI <--> WS[/agent/ui/ws]
    BFF --> ACCTS[账户与平台适配器]
    BFF --> PNL[PnL/账本聚合]
    BFF --> WH[ClickHouse 仓库服务]
    AGENT --> PROC[Agent 进程管理]
    AGENT --> PROF[版本化 TOML Profile]
    AGENT --> KMS[KMS/审批]
    PROC --> EXEC[采集/研究/执行进程]
```

控制台至少由两个后端边界组成：通用 `/api/*` 和独立的 `/agent/api/*`。Agent 日志与状态通过 `/agent/ui/ws` 推送，不是单纯轮询。

## Agent WebSocket 契约

连接地址：

```text
wss://app.signalx.net/agent/ui/ws
```

已恢复的消息形状：

- `type: "snapshot"`：`payload.agents` 为 Agent 快照。
- `type: "event"`：`payload.event_type` 表示事件类型。
- `agent.updated`、`agent.registered`：更新单个 Agent。
- `agent.log`：包含 `agent_id` 和 `lines`，追加实时日志。
- `profile*` 事件：触发 Profile 查询缓存失效。
- 断线后约 2 秒重连。

Agent 规范化字段至少包括：`agent_id/name/display_state/service/host/remote_ip/agent_type/version/compiled_at/identity_public_key`，以及 CPU、RSS、运行时间和多种行情速率指标。

## 数据与运行面事实

- 数据仓库显示 ClickHouse，按 `mkt_cex`、`mkt_pred`、`mkt_ref`、`mkt_quarantine`、`etl`、`research` 等数据库分层。
- 主要表族包括 `bbo`、`book`、`trade`、`oracle`、`index`、100ms 聚合表、隔离问题表和研究表。
- 控制面存在不可变/版本化 TOML Profile、账户槽位绑定、Profile preview、KMS 状态、进程命令、历史与实时日志。
- 进程类型至少覆盖 recorder、pm-hft、winner-tail-sweep、aggregation 和 TWAP 类工作负载。
- 市场数据页面仍是占位页，说明当前系统重心是后台采集、Agent 运维和 PnL/仓库可观测性。

## 无法从前端恢复的内容

- 服务端语言、仓库目录和部署清单。
- 交易所/预测市场适配器实现。
- ClickHouse DDL、物化视图和数据校验 SQL。
- 回测撮合器、队列模型、费用/结算模拟器。
- winner-tail-sweep 等策略的完整实现、全部特征、资金管理和风控阈值。授权控制台已
  暴露部分运行配置与行为，但这些证据不等于完整算法，也不应直接作为复建默认参数。
- 密钥、账户凭证、KMS 数据和私有 Profile 内容。
- 账本、对账和 PnL 归因的服务端实现。

## 清洁室复建建议

第一阶段只实现公开接口的等价能力：

```text
apps/
  console-web/          React 控制台
services/
  api-gateway/          会话、账户、PnL、仓库查询
  agent-control/        Agent 注册、状态、日志、Profile 版本
  market-ingest/        CEX/预测市场/预言机原始数据
  replay-backtest/      事件驱动回放、撮合、费用与结算
  execution-core/       paper/live 共用策略接口和 OMS
  risk-ledger/          限额、账本、NAV 与逐笔对账
packages/
  contracts/            API、事件和数据模型
  strategy-sdk/         策略生命周期与信号接口
  venue-adapters/       各平台隔离适配器
infra/
  clickhouse/
  observability/
```

优先级：先建立 `contracts + market-ingest + replay-backtest`，再做 `agent-control + console-web`；控制台不是最先决定 Alpha 真假的部分。

## 向作者索取源码的最小授权包

如果作者同意协助，建议不要索取完整生产仓库，而先要一个明确授权、已脱敏的只读包：

1. ClickHouse DDL 和字段字典，不含生产连接信息。
2. 策略 SDK/接口定义，不含真实策略实现和参数。
3. 回测事件格式、撮合与结算规则测试用例。
4. Agent 注册、心跳、日志和 Profile 的 OpenAPI/JSON Schema。
5. 一份脱敏的端到端录制样本。
6. 开源许可或书面说明：哪些文件可用于参考，哪些只能用于行为对照。

即使拿到授权包，也应保留独立仓库、提交记录和需求来源，避免混入作者或对象雇主的私有代码、数据和参数。
