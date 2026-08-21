# 目标数据源与连通性情况说明

版本：v0.1｜更新时间：2026-08-18
用途：记录清洁室复建项目的数据源范围、接入状态、证据、阻塞项和排查步骤。

## 1. 当前结论

目标数据源已有明确清单，但确认程度不同，应区分为三类：

1. **已由本地脚本实际接通**：Binance、Polymarket 公共行情。
2. **正式契约已找到、但尚未本地复现**：Predict.fun。
3. **在 SignalX 公开行为或控制台中观察到，但契约仍不完整**：Chainlink Data Streams、Deribit。
4. **内部存储/归档目标，不能当作原始市场源**：Cloudflare R2、ClickHouse。

2026-08-18 的接口不通问题已定位为 **系统 DNS 返回错误地址**。项目已增加
DNS-over-HTTPS（DoH）解析，Binance REST、Binance WebSocket、Polymarket
Gamma API 和 Polymarket Market WebSocket 均已恢复，不需要修改整台机器的
DNS 设置。

## 2. 目标数据源清单

| 优先级 | 数据源 | 目标用途 | 已知接口/数据 | 鉴权 | 当前状态 | 下一步 |
|---|---|---|---|---|---|---|
| P0 | Binance Spot | 基准价格、成交、深度、BBO、时钟对照 | REST `/api/v3/time`；WS `trade`、`depth@100ms`、`bookTicker` | 公共行情无需鉴权 | **已接通并实采** | 连续采集 30 分钟以上；校正本机时钟；建立频率和断序基线 |
| P0 | Polymarket | 目标 venue：主数据、订单簿、价格变化、成交价 | Gamma、Data API、CLOB read、Market Channel | 公开读取无需鉴权；交易需 L1/L2 | **四类只读接口已接通并实采；execution 阻塞** | 固定合约并运行 24h；交易前实际出口 IP geoblock/账户/合规确认 |
| P0 | Predict.fun | 目标 venue：行情、市场生命周期与执行对照 | Testnet/Mainnet REST；`wss://ws.predict.fun/ws` | Testnet 无 API key；主网 key 默认 240 req/min；个人操作需 JWT | **契约已确认，缺授权回复与本地样本** | 实现 Testnet/read-only；申请 key；写接口默认关闭 |
| P1 | Chainlink Data Streams | 结算/预言机参考价格和跨源校验 | SignalX 中观察到 7 个 feed | 需要授权账户/凭据 | **缺凭据与 feed ID** | 确认 7 个 feed 的名称、ID、网络、时间戳和授权方式 |
| P1 | Deribit | BTC/ETH 衍生品参考价格、波动率和期限结构 | 已观察到 Deribit 数据链路，具体频道未固化 | 公共行情通常无需鉴权 | **待契约确认** | 确定 instrument/channel 清单和时间戳语义，再做只读采集器 |
| P2 | Gemini | CEX top-of-book、成交和跨源参考 | 运行日志观察到 Gemini WebSocket top-of-book/trades | 公共行情通常无需鉴权 | **已观察，非首期目标** | 仅在 Binance 单源失效或跨源校验需要时接入 |
| P2 | Kalshi / Polymarket.US | 事件合约 venue 对照 | 控制台存在账户适配器和历史 PnL；当前非主要收益来源 | 账户鉴权且有地区/资格要求 | **适配器存在，目标未确认** | 保持在合规评审后再决定，不进入首期执行范围 |
| P2 | Cloudflare R2 | 原始数据归档、回放和批量回补 | 对象存储归档路径 | 需要 bucket/account 只读权限 | **待授权与目录清单** | 获取脱敏 object key 样例、分区规则、压缩和 schema 版本 |
| P2 | ClickHouse | 标准化数仓、聚合、质量与研究查询 | 已观察到约 43 张表；`mkt_cex`、`mkt_pred`、`mkt_ref`、`mkt_quarantine`、`etl`、`research` 等分层 | 内部访问 | **结构可部分推断，DDL 未取得** | 索取 DDL/字段字典；用本地实采 payload 验证字段，而不是照抄推断 |
| P3 | NOAA | 天气事件参考数据 | 配置 schema 中存在 `ingest.noaa`，当前禁用 | 待确认 | **仅见配置入口** | 有明确天气策略需求后再研究 |

SignalX 自有 `/api/*`、`/agent/api/*` 和 `/agent/ui/ws` 属于控制面与行为对照，
不是本项目应依赖的原始行情源。接口清单见
[`api-surface.yaml`](./api-surface.yaml)。

## 3. 已确定的原始层结构

本地原始层使用一行一事件的 NDJSON。统一字段已经确定：

```text
schema_version
record_kind
session_id
source
stream
instrument
event_type
source_event_ts_ms
source_trade_ts_ms
recv_wall_ts_ms
recv_mono_ns
arrival_latency_ms
snapshot_age_ms
sequence_start
sequence_end
payload
```

原始层可以立即使用，不必等待最终 ClickHouse DDL。后续表结构建议从实采数据归纳为：

```text
dim_instrument
raw_market_event
fact_trade
fact_book_snapshot
fact_book_delta
fact_bbo
fact_reference_price
fact_source_connection
fact_latency_sample
fact_data_quality
```

当前不能最终确定的是 Predict.fun/Chainlink 字段、跨平台 instrument 映射、
订单簿增量恢复规则和 ClickHouse 分区/排序键。这些必须依赖真实样本或正式契约。

## 4. 2026-08-18 连通性故障记录

### 4.1 症状

- Binance WebSocket 在打开前关闭，code `1006`。
- Polymarket Gamma API 超时。
- 浏览器访问 Polymarket 返回 `ERR_CONNECTION_RESET`；Binance 请求挂起。
- Node 环境没有配置 `HTTP_PROXY`、`HTTPS_PROXY` 或 `ALL_PROXY`。
- 本机常见代理端口 `7890/7891/7897/1080/1087/8080/8888` 均未监听。

### 4.2 分层证据

普通控制站点 `example.com` 的 DNS、TLS 1.3 和 HTTP 200 均正常，说明不是完全
断网。目标域名的系统 DNS 与 Cloudflare DoH 对照如下；IP 只代表当次诊断，
CDN 地址以后可能变化。

| 域名 | 系统 DNS 当次结果 | DoH 当次结果 | 判断 |
|---|---|---|---|
| `api.binance.com` | `210.56.51.193`，此前还出现 `199.16.156.7` | `108.158.2.161` | 系统 DNS 错误/不稳定 |
| `stream.binance.com` | `31.13.85.53` | 多个 AWS 东京地址 | 系统 DNS 错误 |
| `gamma-api.polymarket.com` | `128.242.240.91`，此前还出现 `69.171.224.40` | `104.18.34.205`、`172.64.153.51` | 系统 DNS 错误/不稳定 |
| `ws-subscriptions-clob.polymarket.com` | `199.59.150.39` | `104.18.34.205`、`172.64.153.51` | 系统 DNS 错误 |
| `data-stream.binance.vision` | AWS 东京地址 | 相同地址集合 | DNS 正常，但部分地址连接质量不稳定 |

使用 `curl --resolve` 强制 DoH 返回的正确 IP 后：

- Binance REST `/api/v3/time`：HTTP 200。
- Binance WebSocket URL：TLS 成功，普通 HTTP 请求返回 400，符合非升级请求预期。
- Polymarket Gamma Markets：HTTP 200。
- Polymarket Market WebSocket URL：TLS 成功，普通 HTTP 请求返回 400，符合预期。

因此根因不是接口下线；主要问题是系统 DNS 污染，辅以跨境链路时延和个别 CDN
地址可达性差。

### 4.3 项目内修复

- 增加系统 DNS 与 Cloudflare DoH 对照诊断。
- HTTP 请求支持自定义 DNS lookup。
- WebSocket 改用 `ws` 客户端并注入自定义 lookup。
- 采集命令默认 `--dns doh`；可在可信网络使用 `--dns system`。
- DoH 结果按 TTL 缓存，并在同一域名的多个 A 记录间轮换。
- 未修改 macOS 系统 DNS，避免影响其他应用。

当时的 Node 探针实现已经完成验证，并于白皮书 v0.2 精简仓库时从主分支移除；需要时可从
Git 历史恢复。保留的本地验证证据已迁入 `exploration/local-data/verification/`，大文件不进入 Git。

## 5. 修复后实测

修正事件过滤后的最终 15 秒 Binance + Polymarket 双源实采结果：

- 总记录：2,364 行，invalid JSON 0。
- Binance：open 1、close 1、error 0、parse error 0。
- Polymarket：open 1、close 1、error 0、parse error 0。
- Binance depth：92 条；trade：709 条；bookTicker：1,557 条。
- Polymarket：两个订阅资产各收到一份初始 book snapshot；全局 `new_market`
  噪声已经过滤。
- Binance trade 校正后到达延迟：P50 约 724 ms，P95 约 1,403 ms，P99 约
  1,404 ms。
- Binance depth 校正后到达延迟：P50 约 283 ms，P95 约 1,067 ms，P99 约
  1,308 ms。
- 较早的低流量 15 秒样本中 trade P50/P95 约为 280/284 ms、depth P50/P95
  约为 278/299 ms。两次短样本差异很大，说明当前链路存在明显抖动或批量到达，
  不能挑选单次最好结果作为基线。

以上只是 15 秒连通性样本，不能作为生产 SLA。尤其需要注意：

- 本机时钟不稳定：第一次 Binance 探针约慢 **1.12 秒**，后续约慢
  **2.31–2.33 秒**；直连 Apple NTP IP 测得约慢 **2.33 秒**。必须先修复系统时间
  同步；当前校正值只能用于与采集紧邻的短样本。
- Binance `bookTicker` 没有源事件时间戳，只能用于接收频率和 BBO 状态，不能
  计算单向到达延迟。
- Polymarket 初始 `book` 时间戳表示快照新鲜度，已单独记为 `snapshot_age_ms`，
  不再混入实时到达延迟。
- 15 秒 Polymarket 活跃合约样本只收到初始快照，实时事件延迟仍需更长采集窗口。

新增的公共只读快照工具已在 2026-08-18 实际跑通 Gamma market discovery、两个
CLOB REST orderbook 和 Data API recent trades；一次小样本写入 1 条 market metadata、
2 条 book、5 条 public trade，共 8 条 NDJSON。样本保存在 Git 忽略目录，只用于推导
canonical schema，不包含任何本项目账户凭据或订单写请求。

## 6. 日常排查命令

```bash
cd "/Users/chenglitao/Desktop/work_project/二元期权量化/benchmark"
npm install

# 快速比较系统 DNS 和加密 DNS
npm run diagnose:network -- \
  --dns-only \
  --output data/diagnostics/dns-latest.json

# 完整 DNS/TLS/HTTP/WebSocket 分层诊断
npm run diagnose:network -- \
  --timeout 8000 \
  --output data/diagnostics/network-latest.json

# 时钟偏差
npm run probe:clock -- --samples 10 --dns doh

# 双源采集
npm run collect:public -- \
  --duration 1800 \
  --symbol BTCUSDT \
  --polymarket-query bitcoin \
  --dns doh

# Gamma metadata + CLOB REST books + Data API public trades
npm run snapshot:polymarket -- --query bitcoin --trade-limit 100 --dns doh
```

## 7. 仍需人工决策或外部输入

1. **本机时间同步**：`com.apple.timed` 正在运行，但按域名执行 `sntp
   time.apple.com` 出现 DNS lookup failure；直接查询 DoH 返回的 Apple NTP IP
   可以成功，测得约 `+2.33 s` offset。需要管理员检查“自动设置日期与时间”并
   为系统配置可靠 DNS；在偏差稳定低于 10 ms 前，不发布正式单向延迟结论。
2. **双 venue 合约选择**：不能长期用关键词后自动选成交额第一名；需要固定
   market ID/asset IDs，并定义换月或到期切换规则。
3. **benchmark 口径**：确定采集地域、持续时间、P50/P95/P99 门槛、断序率、
   重连率和允许的时钟误差。
4. **Predict.fun**：正式端点、Testnet 无 key、主网 key 默认 240 req/min 和 15 秒 heartbeat 已确认；
   官方基础设施信息指向 `ap-northeast-1`。仍需申请最小权限 key、确认账户/使用
   条款并取得实际消息样本。
5. **Chainlink**：提供授权方式与 7 个 feed ID；密钥不得写入仓库。
6. **Deribit**：确认需要 spot index、perpetual、options 还是 volatility index。
7. **部署位置**：DoH 解决了接口可达性，但当前约 280 ms 的 Binance 到达延迟
   不适合作为低延迟生产节点；需要决定东京/新加坡等靠近数据源的 benchmark 节点。
8. **执行 benchmark 口径**：作者提出“直接对着下单接口测”，需进一步确认是
   HTTP ACK、用户流确认还是 first fill，以及是否使用有效订单和预热连接。详细矩阵见
   [`OBSERVABLE_SURFACE_FINDINGS_20260818.md`](./OBSERVABLE_SURFACE_FINDINGS_20260818.md)。
9. **Polymarket execution**：官方只读接口无需鉴权，但写接口需钱包/L1/L2；实际出口
   IP 必须先通过 geoblock 检查，不能以更换云区域规避限制。

## 8. 官方协议参考

- [Binance Spot WebSocket Streams](https://developers.binance.com/en/docs/binance-spot-api-docs/web-socket-streams)
- [Polymarket Market Channel](https://docs.polymarket.com/market-data/websocket/market-channel)
- [Polymarket WebSocket Overview](https://docs.polymarket.com/market-data/websocket/overview)
- [Polymarket API Overview](https://docs.polymarket.com/api-reference/introduction)
- [Polymarket Authentication](https://docs.polymarket.com/api-reference/authentication)
- [Polymarket Geographic Restrictions](https://docs.polymarket.com/api-reference/geoblock)
- [Polymarket Quickstart](https://docs.polymarket.com/quickstart)
- [Predict.fun Developer Documentation](https://dev.predict.fun/)
