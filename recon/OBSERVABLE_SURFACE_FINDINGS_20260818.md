# SignalX 可观察面深挖报告

版本：v0.1｜观察日期：2026-08-18｜来源：授权控制台、当前公开前端包、运行日志

## 1. 执行摘要

这轮观察显著收窄了项目边界：

- 核心运行进程是 Rust 服务，模块路径以 `pm_hft::*` 为主，覆盖 feed、segment、
  engine、alpha、execution、order tracker 和 user stream。
- 系统的主要实盘优势不是一般意义上的“行情更快”，而是结算/参考结果先确认后，
  立即向预测市场提交订单的端到端速度。
- 作者所说“直接对着下单接口测”对执行延迟 benchmark 是合理的，但若只测 HTTP
  RTT，仍不足以代表策略竞争力。
- 数据链路基本确定为 Agent 分片采集 → 本地 segment → R2 → ETL → ClickHouse；
  ClickHouse 当前主要承载市场数据、研究数据与日志，不是完整交易账本。
- 仓库约 150.14 亿行、394 GB 压缩数据、2.5 TB 文件系统占用；按当前在线 Agent
  显示的约 3,026 events/s 粗算，对应约 57 天积累量，和配置中 6 月底至 8 月的
  研究窗口相符。
- 数据量很大，但质量治理尚不成熟：约 6.66 亿行进入 quarantine，时间戳延迟和
  覆盖率页面暂无数据，部分 trade 查询失败率和尾延迟明显偏高。

本文只记录行为和接口证据。控制台展示的凭据、完整地址、账户 ID 和身份材料均未
复制。策略中的具体资金值仅用于解释测量路径，不作为目标项目默认参数。

## 2. “对着下单接口测”是否合理

### 2.1 为什么在这个项目中合理

授权日志显示 `winner_tail_sweep` 的核心顺序是：

1. 从 venue GraphQL 获取当轮 start price。
2. 在结算时并发请求 end price，采用“第一个成功响应”。
3. 确定获胜 outcome 后，提交接近结算价的 LIMIT order。
4. 通过订单轮询和 user WebSocket 跟踪 live、cancel、fill 状态。
5. 每 30 秒并行预热 8 条 Predict.fun REST 连接。

账户活动也显示 5 分钟 up/down 合约在临近关盘时发生 CREATE/INVALIDATE。由此可见，
策略的竞争窗口确实落在“结果确认 → 下单”之间。服务器地域和网络优化应优先对着
真实订单入口测量，而不是只 ping 域名或测公共行情 WebSocket。

### 2.2 这句话缺少的定义

“下单接口延迟”至少可能指四件不同的事：

| 指标 | 起点 | 终点 | 能回答什么 |
|---|---|---|---|
| HTTP RTT | 开始写请求 | 收到完整响应 | 网络、网关和同步处理速度 |
| order ACK | 订单意图生成 | API 返回已接受/拒绝 | 客户端签名、连接、风控和服务端受理 |
| user-stream confirm | 订单意图生成 | 用户流出现订单状态 | 下单路径与异步状态链路是否一致 |
| first fill | 订单意图生成 | 首个成交确认 | 最接近实际竞争结果，但受价格和队列影响 |

如果用无效签名、缺字段或远离盘口的订单测量，只能证明请求到达；它可能绕过账户
风控、撮合和用户流路径。若用真实可成交订单测量，又会引入资金风险、市场冲击和
样本选择偏差。因此必须把网络、有效 ACK、用户流和成交分别报告。

### 2.3 推荐的时间点

```text
t0 参考/结算结果首次可用
t1 策略完成 outcome 判定
t2 订单完成序列化与签名
t3 请求字节写入已预热连接
t4 HTTP order ACK/拒绝返回
t5 user WebSocket 确认订单状态
t6 公共订单簿反映订单（若可观察）
t7 首次成交或取消确认
```

核心分布应包括 `t1-t0`、`t3-t1`、`t4-t3`、`t5-t3`、`t7-t1`，并同时记录错误率、
状态不一致、重试、限流和未确认订单。`t4-t3` 使用 monotonic clock 测 RTT，不依赖
两端时钟同步；跨源的 `t1-t0` 才需要可信 NTP/PTP。

### 2.4 正式 benchmark 矩阵

- 东京节点优先测试 Predict.fun；伦敦仅在合规范围内测试 Polymarket。
- 分开记录冷连接、单条 keep-alive、预热连接池和并发连接池。
- 第一层只做认证读取/协议探针；第二层做有效但不成交的 maker create/cancel；第三层
  才是在明确批准下做极小额度 IOC/FOK 或 canary。
- 每种场景至少覆盖 24 小时和高峰/结算窗口，报告 P50/P95/P99/max 与失败率。
- 保留服务端 request/order ID，使 API 响应、用户流、公共簿和本地日志能够关联。
- 同时测 end-price/参考源；只测订单入口会漏掉本策略真正的另一个竞速点。

在取得平台授权、账户资格和额度审批前，不运行真实订单 benchmark。

## 3. 可见系统规模

### 3.1 控制面

| 项目 | 当次观察 |
|---|---:|
| 交易账户 | 13 |
| 账户/venue 类型 | Predict.fun、Polymarket、Polymarket.US、Kalshi、Binance、Deribit |
| Agent | 17 |
| 在线 Agent | 5 |
| 启用配置档案 | 10 |
| 在线主机下限 | 3 台 AWS EC2（东京 1、伦敦 2） |

五个在线 Agent 显示的事件总速率合计约 3,026 events/s。Agent 版本均显示 `0.1.0`，
但编译时间、运行时间和配置版本不同，说明二进制版本号目前不足以唯一标识部署物。

### 3.2 ClickHouse 总览

| 指标 | 当次观察 |
|---|---:|
| 表 | 43 |
| 行数 | 15,014,449,023 |
| 压缩后数据 | 394 GB |
| 文件系统占用 | 2.5 TB / 14.4 TB（17.1%） |
| 服务运行时间 | 11d 4h |
| 版本 | ClickHouse 26.5.3.52 |

服务运行时间只表示本次 ClickHouse 进程启动时间，不能当作数据保留期。

### 3.3 行数构成

| 数据库族 | 估算行数 | 占总行数 |
|---|---:|---:|
| `mkt_cex` | 9.930B | 66.14% |
| `mkt_pred` | 2.945B | 19.61% |
| `mkt_ref` | 1.438B | 9.57% |
| `mkt_quarantine` | 0.666B | 4.44% |
| `research` | 23.6M | 0.16% |
| `etl` | 10.9M | 0.07% |
| `obs` | 1.16M | 0.01% |

### 3.4 主要表与压缩密度

以下 bytes/row 用页面显示的十进制 GB 粗算，只用于容量外推：

| 表 | 行数 | 压缩大小 | 约 bytes/row |
|---|---:|---:|---:|
| `mkt_cex.bbo` | 7.192B | 103 GB | 14.3 |
| `mkt_cex.book` | 1.531B | 101 GB | 66.0 |
| `mkt_pred.bbo` | 1.588B | 77 GB | 48.5 |
| `mkt_pred.book` | 1.178B | 32 GB | 27.2 |
| `mkt_quarantine.pred_bbo_lon_problem` | 637M | 29.8 GB | 46.7 |
| `mkt_cex.bbo_100ms` | 638M | 14 GB | 22.0 |
| `mkt_ref.oracle` | 40.0M | 11.9 GB | 297.4 |
| `mkt_ref.ref_price_archive` | 1.392B | 8.9 GB | 6.4 |
| `mkt_cex.trade` | 340M | 6.5 GB | 19.1 |
| `mkt_pred.trade` | 179M | 3.0 GB | 16.7 |

前十张表约占压缩数据 98%，容量规划不需要一开始精确复刻所有 43 张表。

## 4. 对体量和保留期的反推

### 4.1 数据年龄

以当前五个 Agent 的 3,026 events/s 恒定外推：

```text
15,014,449,023 / 3,026 / 86,400 ≈ 57.4 天
```

配置和研究表出现 6 月 29 日至 8 月 18 日窗口，因此“约 6–8 周的积累量”是合理
估计。该估计有较大误差：历史 Agent 数、订阅标的、市场活跃度、重复/替换行、回补
和 quarantine 都会改变速率。

### 4.2 日增长量

若 394 GB 压缩数据对应 40–60 天，逻辑表数据约增长 6.6–9.9 GB/天。若把 2.5 TB
文件系统占用也全部归因于同一窗口，则约为 42–64 GB/天，但这个上界很可能混入
本地 raw segment、合并临时空间、detached part、缓存和其他文件。

因此当前只能给两层规划：

- ClickHouse 活跃数据：先按 **10–15 GB/天压缩增长** 留余量。
- 节点文件系统：在服务器上实测 `data/metadata/log/tmp/detached` 分类后再设容量告警；
  不能用 394 GB 直接解释 2.5 TB。

### 4.3 对象与 ETL

`etl.ingested_object` 有约 1,086 万行，配置存在 `segment.r2` 账户槽位，Agent 日志存在
`market::segment::sink` 与内存压力触发提前 seal。这强烈支持以下链路：

```text
Agent 内存批次 → 本地 segment seal → R2 object → ETL 幂等登记 → ClickHouse
```

平均每个 ingested-object 记录对应约 1,380 行市场事实，但一次对象可能产生多张表、
重试或回补记录，不能直接把这个数字当作对象内事件数。

## 5. 仓库结构和成熟度判断

### 已确定

- 原始事实表广泛使用 `ReplacingMergeTree`，符合对象重放和幂等去重需求。
- `bbo_100ms` 使用 `AggregatingMergeTree` 并有 MaterializedView 骨架。
- 数据按 `mkt_cex/mkt_pred/mkt_ref/mkt_quarantine/research/etl/obs/trd/dim` 分层。
- 研究层存在 Binance spot/perp 100ms、预测市场 200ms、Chainlink 1s 和 fill feature 表。
- Agent 日志通过 ClickHouse 最近 100 行 + WebSocket 增量展示。

### 暴露的问题

- quarantine 约 6.66 亿行；仅 `pred_bbo_lon_problem` 就有 637M 行/29.8 GB。
- 仓库页面明确显示“暂无覆盖率数据”和“暂无时间戳延迟数据”。
- `mkt_cex.trade` 当日查询失败率 44.9%，P95 5.51s；`mkt_pred.trade` P95 14.21s。
- `dim.instrument/run/stream` 和 `trd.order_event/order_state/settlement/signal` 均为 0 行。
- `obs.span`、`obs.stream_stat` 为空；现阶段观测主要依赖日志，而不是完整 trace/stream SLO。

所以 150 亿行证明了采集与存储规模，但不能证明数据完整、延迟可信或交易账本成熟。
目标项目应优先补质量指标和可重建性，不照搬现有数据量。

## 6. 暴露出的运行与策略边界

### 数据源和协议

- Binance spot trade、depth、book ticker，且日志明确出现 Spot SBE session。
- Binance futures 以及 BTC/ETH/SOL/XRP/DOGE/HYPE/BNB 等订阅配置。
- Polymarket Gamma、CLOB WebSocket 和 RTDS。
- Predict.fun REST、WS、GraphQL，并支持配置固定 GraphQL origin。
- Chainlink Data Streams、Deribit raw ticker、Gemini top-of-book/trades。
- Kalshi、Polymarket.US 账户适配器；NOAA ingest 配置存在但处于禁用状态。

### 运行模式

- `record-v2`：市场录制与 R2 segment 输出。
- `run`：实盘策略生命周期与执行。
- 独立 CEX + Chainlink recorder 正迁移到 `pm-hft record-v2`，表明采集框架在统一。
- Profile 主体、账户槽位和凭据引用分离；发布新版本不可变，Agent 绑定到具体版本。

### 时间粒度

- 通用运行 tick 多为 100ms；结算抢单配置出现 10ms tick。
- 市场 prefetch/post-expiry 窗口均出现 30s。
- 回测 tape 只截取结算前 3s、结算后 5s，最大并发 24。
- 研究表使用 100ms/200ms/1s 多级采样，而不是所有研究都扫逐事件原始表。

这些信息足以设计兼容的配置 schema、回放窗口和 collector/strategy 生命周期；不
足以复制私有信号公式、风控阈值和撮合细节。

## 7. 实盘行为观察

授权 PnL 页面显示：

- 2026 年 7 月：+$1,064.84，1,405 条已结算记录，23 个活跃日中 15 个盈利日。
- 2026 年 8 月截至 18 日：+$886.02，1,319 条已结算记录，18 个活跃日中 12 个
  盈利日。
- 盈亏高度集中在 Predict.fun；其他 venue 当期贡献接近零或为小额负数。
- 主账户最近 1 天显示 55 个已结算市场和约 8.65 万条本地活动事件。

这些是账户聚合页面的结果，不是审计后的策略收益率：资金流、奖励、未结算风险、
重复事件和费用归因仍需单独验证，不能直接用来宣传策略表现。

## 8. 下一轮向作者追问

1. “直接对着下单接口测”的起止点是 HTTP ACK、用户流确认还是 first fill？
2. 用的是真实有效订单、maker create/cancel，还是无效/模拟请求？
3. 是否预热连接；连接池大小、HTTP 版本、签名和序列化是否计入？
4. benchmark 在哪个 region、持续多久、样本多少，P50/P95/P99 和失败率是多少？
5. 参考结果/settlement API 的时间是否也测量，如何关联到 order request ID？
6. 能否提供脱敏 benchmark 脚本和一份原始输出，而不是只给汇总数字？
7. ClickHouse 394 GB 与 `/var/lib/clickhouse` 2.5 TB 的差额分别是什么目录？
8. 大表的最早/最晚时间、TTL、partition/order key、dedup/version 列是什么？
9. `*_lon_problem` 的进入条件和修复状态是什么？是否能回填主表？
10. R2 object 的分区、segment seal 条件、校验和和 ETL exactly-once 语义是什么？

## 9. 尚不能确定

- 下单入口的服务端处理阶段、撮合队列位置和订单公平性机制。
- ClickHouse 是单机还是控制台聚合后的集群视图；是否有副本和灾备。
- R2 原始对象的真实体积、保留期和压缩格式。
- 150 亿行中重复版本、回补和实际唯一事件的比例。
- 策略的全部信号、资金管理、费用、奖励和尾部风险。
- 空 `trd.*` 表是否代表未上线、另有账本，还是仅未同步到 ClickHouse。
