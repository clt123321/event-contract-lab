# 产品与系统需求

版本：v0.3｜状态：Draft｜仓库：`event-contract-lab`

## 1. 产品定义

建设一套面向事件合约和预测市场的模块化量化研究与交易工程平台，支持多源行情
采集、统一事件模型、可重复回放、策略研究、paper/live 共用执行接口、风险账本、
Agent 运维和审计。

首要问题不是复刻某个界面，而是回答并持续验证：数据是否完整、延迟是否可用、
策略是否在真实费用和执行约束下仍有优势、实盘状态是否可审计。

## 2. 原则与边界

### 2.1 必须遵守

- 清洁室复建：只使用公开协议、授权可观察行为和自行采集样本。
- 原始数据不可变；所有标准化和派生字段可追溯、可重算。
- wall clock 与 monotonic clock 分开；单向延迟必须记录时钟误差边界。
- paper 与 live 使用相同策略生命周期和订单接口，但 live 默认关闭。
- 凭据只进入密钥系统或运行时环境，不进入 Git、日志和测试夹具。
- 平台地域、账户资格和数据使用条款是部署约束，不得用代理或云区域规避。

### 2.2 明确不做

- 不复制或反编译获取目标系统私有源码、策略参数、密钥和生产配置。
- 不把 15 秒烟雾测试包装成生产 SLA。
- 不在缺少回放、风控、对账和 kill switch 时启用实盘。
- 不以控制台 UI 完成度替代数据和执行正确性。

## 3. 用户与使用场景

| 用户 | 核心任务 |
|---|---|
| 研究员 | 获取可复现数据集，开发信号，运行事件驱动回放 |
| 策略工程师 | 将策略接入 paper/live 共用 SDK，查看延迟和成交归因 |
| 运维人员 | 部署 Agent、查看状态/日志、切换不可变配置、处理告警 |
| 风险负责人 | 设置限额、审批实盘、审计订单、持仓、余额和 PnL |

## 4. 功能需求

### FR-100 数据接入与时钟

- `FR-101 CONFIRMED`：接入 Binance trade、depth、bookTicker。
- `FR-102 CONFIRMED`：接入 Polymarket Gamma、Data API、CLOB read 和 Market Channel；
  只读接口不因其被列为目标 venue 而升级为交易授权。
- `FR-103`：接入 Predict.fun Testnet/主网授权 REST/WS；只读阶段不得调用订单写接口。
- `FR-104`：接入 Chainlink Data Streams 指定 feed。
- `FR-105`：接入 Deribit 指定 index/perpetual/options channel。
- `FR-106`：每条事件记录源时间、接收 wall clock、接收 monotonic clock和序列信息。
- `FR-107`：采集器必须实现心跳、退避重连、重订阅、断序检测和原始 payload 保留。
- `FR-108`：部署前必须验证 DNS、TLS、WebSocket 和 NTP/PTP。
- `FR-109`：本地发布候选和部署后 smoke 必须复用版本化阈值及机器可读报告；每次调整
  生成新报告，不覆盖或人工修改历史结果。
- `FR-110`：source role 必须显式区分 execution venue、reference、oracle 和 research；
  Predict.fun/Polymarket 为目标 venue，Binance 为首个 reference source。

验收：连续 24 小时无静默断流；重连和断序均有记录；源时间缺失不会伪造延迟。

### FR-200 数据契约与数仓

- `FR-201 CONFIRMED`：统一原始事件 envelope 版本化。
- `FR-202`：建立 instrument、market、outcome 跨源映射。
- `FR-203 IMPLEMENTED LOCAL`：建立 trade、book snapshot、book delta、BBO、reference price
  事实表；当前 Canonical v1 已覆盖前四类，reference price 专用事实仍待实现。
- `FR-204`：建立 source connection、latency sample、data quality 事实表。
- `FR-205 PARTIAL`：NDJSON 和确定性 ZSTD Parquet 本地存储已实现；R2 冷归档与 ClickHouse 热查询待完成。
- `FR-206 IMPLEMENTED LOCAL`：隔离无效时间戳、重复、序列回退、BBO/完整快照交叉、
  概率越界和不可解析 book；warning 与 quarantine 均有版本化计数。
- `FR-207`：数据分为 Raw/Bronze、Canonical/Silver、Serving/Gold，后两层均可由上游重建。
- `FR-208 PARTIAL`：已建立逐行 Raw→canonical lineage、transform manifest 和 Dataset
  Manifest v2；Gold lineage 与多分区 dataset registry 在 P2 完成。

验收：任意事实行可回溯到原始事件；同一批数据重复转换结果一致。

### FR-300 回放与研究

- `FR-301 IMPLEMENTED LOCAL`：按 `available_at_ms` 做 point-in-time 回放，用
  source/session/monotonic/event ID 稳定解决同时刻顺序。
- `FR-302`：模拟费用、结算、盘口冲击、延迟、队列和部分成交。
- `FR-303`：支持 train/validation/test 时间切分和参数实验记录。
- `FR-304`：报告容量、换手、滑点敏感性和统计可信度。
- `FR-305`：point-in-time join 禁止未来数据泄漏；特征记录 as-of time、版本和输入数据集。
- `FR-306`：实验登记 commit、config、dataset、seed、参数、失败结果和完整 artifact。

验收：黄金夹具结果确定；同一提交、配置、数据版本的结果可复现。

### FR-400 策略与执行

- `FR-401`：定义策略生命周期、输入事件、信号、目标仓位和订单意图。
- `FR-402`：paper/live 共用 OMS 接口，venue adapter 隔离平台差异。
- `FR-403`：订单状态机覆盖提交、确认、部分成交、成交、撤单、拒绝和结算。
- `FR-404`：支持幂等 client order ID、超时、重试和未知状态恢复。
- `FR-405`：任何实盘路径必须经过账户、市场、金额、库存和日损限额。

验收：paper 环境通过故障注入；live canary 必须由负责人单独批准。

### FR-500 风险、账本与对账

- `FR-501`：维护订单、成交、余额、持仓、费用和结算的双向可追溯账本。
- `FR-502`：区分交易 PnL、费用、奖励、资金流和估值变化。
- `FR-503`：定时与平台 API/链上状态对账，差异进入隔离队列。
- `FR-504`：提供全局和账户级 kill switch。

验收：余额和持仓不一致会阻断新风险敞口；所有人工修正可审计。

### FR-600 Agent 控制与可观测性

- `FR-601`：Agent 注册、心跳、版本、主机、资源和数据速率可见。
- `FR-602`：配置不可变、版本化、可预览、可审计。
- `FR-603`：日志、指标和 trace 使用统一 session/order/correlation ID。
- `FR-604`：告警覆盖断流、时钟漂移、积压、重连风暴、磁盘和风险门限。

验收：不登录服务器即可判断 Agent 是否健康以及数据停在哪一段。

### FR-700 容量、成本与数据生命周期

- `FR-701`：每个运行环境必须记录实例、磁盘、对象存储、网络和观测成本标签。
- `FR-702`：按 source/stream 输出 events/s、raw bytes/day、compressed bytes/day、
  对象数、压缩比和 quarantine 比例，并能外推 30/90/365 天容量。
- `FR-703`：WAL 默认保留 24–72 小时，ClickHouse 默认热存 30–90 天；长期原始数据
  进入不可变对象存储，保留期由 D-006 批准。
- `FR-704`：对象以 64–256 MB 或等价时间窗 seal，禁止无上限小对象增长；每个
  segment 有 checksum、schema version、source/time range 和重放 manifest。
- `FR-705`：计算、热盘、快照、对象存储、流出流量和 secret 必须分别预算并设告警。
- `FR-706`：网络或地域升级必须由 order ack/user stream/fill 的 paired benchmark
  证明，不以 ICMP ping 或实例标称带宽代替。
- `FR-707`：购买 1/3 年承诺前必须积累至少 30 天利用率与容量曲线，实验节点不得承诺。

验收：能够用同一份输入重算月/年成本；预算偏差超过 25% 有归因；对象存储样本可
恢复到空 ClickHouse；扩容和长期购买均关联 benchmark 与人工审批。

### FR-800 DFX 工程能力

- `FR-801`：提供一键 bootstrap、统一任务入口、本地 mock、fixture 和示例配置。
- `FR-802`：建立 unit、contract、integration、property/fuzz、黄金回放和故障注入测试。
- `FR-803`：CI 覆盖格式、lint、类型、测试、依赖/许可证、secret、SAST、SBOM 和镜像扫描。
- `FR-804`：部署由 IaC、预检、不可变配置、canary、回滚和 runbook 管理。
- `FR-805`：定义数据/服务 SLO、RPO/RTO、budget 和性能回归门禁并保留演练证据。
- `FR-806`：正式实验与发布具备 code/config/schema/data/artifact provenance。
- `FR-807`：敏感写操作记录 actor、审批、环境、原因、payload hash 和结果，不记录 secret。

验收：新环境 30 分钟内跑通只读采集；CI 能复现网络/限流/乱序/磁盘故障；空库恢复、
部署回滚和 secret 泄漏拦截均有自动化证据。

## 5. 非功能需求

| 编号 | 要求 |
|---|---|
| NFR-01 | 原始事件落盘不能依赖下游数据库可用性 |
| NFR-02 | 所有网络客户端有超时、重试上限和可观测状态 |
| NFR-03 | 生产密钥不得出现在仓库、进程参数、错误栈和普通日志 |
| NFR-04 | schema、配置、策略和部署版本均可关联到 Git commit |
| NFR-05 | benchmark 报告同时给出样本数、窗口、时钟误差和异常计数 |
| NFR-06 | 单源故障不得破坏其他源的采集和原始落盘 |
| NFR-07 | 部署、回滚和数据恢复有自动化脚本与演练记录 |
| NFR-08 | 磁盘剩余低于 30% 或 14 天预测将耗尽时告警；低于 15% 停止非关键回补 |
| NFR-09 | quarantine 超过总事件 1% 或单源基线两倍时告警，不允许静默增长 |
| NFR-10 | 执行 benchmark 至少覆盖 result/event → order ack → user update → fill 分段 |
| NFR-11 | P99/吞吐、schema、兼容性或安全回归超过批准阈值时阻断合并/发布 |
| NFR-12 | 正式数据集和实验不能依赖 notebook 隐式状态或未版本化手工修改 |
| NFR-13 | 所有服务提供 health/readiness、资源水位、版本和配置指纹 |

## 6. 里程碑

| 阶段 | 交付物 | 当前判断 |
|---|---|---|
| M0 探索与契约 | 源清单、原始 envelope、网络与时钟诊断 | 基本完成 |
| M1 数据链路与模型 | 双 venue 只读、WAL、Parquet、质量报告、canonical schema | 进行中：本地闭环已通，多分区/R2/24h soak 待完成 |
| M2 数仓与数据集 | Bronze/Silver/Gold、ClickHouse、manifest、lineage、恢复 | 等 24h 样本定 DDL |
| M3 研究与回放 | 冻结数据集、point-in-time、replay、费用/结算、黄金夹具 | 部分完成：冻结/调度/黄金夹具已通，执行现实性待实现 |
| M4 Paper OMS | 双 venue adapter 接口、订单状态机、模拟执行 | 不启用主网写 |
| M5 DFX/控制面强化 | CI、安全、Agent、IaC、SLO、恢复和审计 | 从 M1 贯穿实施 |
| M6 实盘门禁 | KMS、风控、对账、canary | 当前禁止进入 |

## 7. 完成定义

项目功能只有同时满足以下条件才算完成：需求有编号、实现有测试、数据有来源、
运行有指标、失败有恢复路径、敏感操作有审批、结果能关联代码和配置版本。
