# 开发与部署准备度

版本：v0.3｜评估日期：2026-08-20
结论：**可以开始开发大部分只读数据、回放、paper execution 和控制面骨架；尚不具备大规模生产部署和实盘准入条件。**

云账号和服务器不是当前开发前置条件。当前基础 L3 已能生成 clean release 报告，但不会因此
立即申请服务器；继续在本地完成 Parquet、回放夹具和部署 artifact/IaC 评审后，才申请
14 天短期节点。

## 1. 当前可立即开发的范围

| 工作包 | 准备度 | 说明 |
|---|---:|---|
| 公共行情采集器 | 高 | Binance、Polymarket 已跑通；Predict.fun Testnet/read-only 契约已确认，主网 key 待申请 |
| 原始事件层 | 高 | envelope、时钟字段、不可变原始 payload 已确定 |
| 数据质量与延迟报告 | 高 | 已有采集、网络/时钟诊断、Silver quality/quarantine 和汇总脚本 |
| Parquet/R2 归档 | 中高 | 可先按日期/来源/流分区，生命周期和保留期待定 |
| ClickHouse 模型 | 中 | 可建立候选 DDL；排序键、分区键需用 24h 样本验证 |
| 事件回放引擎 | 中高 | 可先实现确定性调度、黄金夹具、费用与延迟注入 |
| Strategy SDK 与 paper OMS | 中 | 生命周期和状态机可开发；具体 venue 订单语义仍需正式契约 |
| Agent/控制面/可观测性 | 中 | 注册、心跳、配置、日志、指标等通用部分可开发 |
| live execution | 低/禁止 | 缺凭据治理、资格确认、风控、对账、kill switch 和 canary 审批 |

建议先完成 M1 数据链路与模型，再进入 M2 数仓和 M3 研究回放。DFX 基线从 M1 同步
建设，不把测试、安全、恢复和可观测性留到最后补。

## 2. 大规模开发前门禁

### G0：证据与范围基线

- [x] 数据源按“已确认、推断、阻塞”分级。
- [x] 公共数据原始 envelope 已版本化。
- [x] 本地 DNS 故障已定位并有项目内绕行方案。
- [ ] Predict.fun 官方工单确认 Testnet create/cancel 频率；主网只读 API key 和消息样本到位。
- [ ] Chainlink 目标 feed ID、Deribit 目标 channel 由业务负责人确认。
- [x] Polymarket 作为目标 venue 接入公开只读数据。
- [ ] Polymarket execution 需实际出口 IP geoblock、账户和合规负责人确认。

### G1：工程基线

- [x] 本地 Git 仓库、忽略规则、需求与架构文档。
- [x] 主技术栈采用 Rust：采集/回放/执行，TypeScript：控制面，SQL/Python：研究。
- [x] 按已确认技术栈建立 monorepo 目录布局。
- [x] 建立 CI：Rust 格式/Clippy/测试、Node 测试、Raw→WAL 集成和 secret scan。
- [x] 建立 schema/config 版本策略、变更审查规则和 ADR 模板。
- [x] 建立测试数据脱敏规则；首个 fixture 为明确标记的 synthetic payload。
- [x] 建立本地/部署后共用验证器、版本化阈值、逐步骤日志和机器可读报告。

当前 CI 已加入 npm 高危漏洞门禁和 RustSec 依赖审计；许可证策略与 schema 自动代码生成将在
后续 DFX 批次加入。这不阻塞只读 Raw/WAL 开发，但在 G2 部署前必须完成。

### L3：本地发布候选门禁

- [x] `make verify-local` 可在 dirty worktree 中持续运行并明确给出 warning。
- [x] `make verify-release` 要求 clean commit，失败以非零状态退出。
- [x] synthetic Raw → WAL → manifest → checksum verify 纳入同一报告。
- [x] synthetic Raw → Canonical Silver → quality/quarantine → transform manifest 纳入同一报告。
- [x] 未来主机 `make verify-host` 复用同一报告格式，覆盖网络、时钟、公共行情和 WAL。
- [ ] Parquet、回放黄金夹具和本地故障注入达到 P1/P2 目标。
- [ ] IaC plan、部署 artifact、版本/回滚策略在不创建云资源的情况下完成评审。

### G2：服务器部署门禁（本地数据/回放候选后）

- [ ] L3 clean release、Parquet、回放夹具和部署 artifact/IaC 评审均通过，然后再申请
  云账号和服务器。
- [ ] 云账号、预算、账单告警和资源负责人明确。
- [x] 首轮 14 天预算上限 $150 已批准；长期资源和 1 年承诺未被提前购买。
- [ ] 东京节点完成不少于 24 小时的只读连续 benchmark；Polymarket execution 必须
  在未来实际出口 IP 上单独完成 geoblock 与资格检查。
- [ ] 节点启用 chrony/云厂商时间服务；正式样本时钟偏差稳定低于 10 ms，并在
  每份报告中记录偏差和采样方法。
- [ ] IaC、最小安全组、非 root 运行账户、磁盘加密、自动补丁策略完成。
- [ ] 原始 WAL 在数据库不可用时仍能落盘；磁盘容量、保留期和归档回补经过演练。
- [ ] 采集报告包含 events/s、raw/compressed GB/day、对象数、压缩比、quarantine
  比例和 30/90/365 天容量外推。
- [ ] 对象存储 segment 目标 64–256 MB，manifest/checksum 可在空库完成恢复演练。
- [ ] DNS、TLS、WebSocket、NTP、重连、断序、磁盘满和进程重启均有监控与告警。
- [ ] 密钥通过云 KMS/Secrets Manager 或等价系统注入，不出现在 Git、命令行和日志。

### G3：实盘门禁

- [ ] 平台账户、地区资格、API 使用方式和资金主体经人工确认。
- [ ] paper/replay 使用同一订单状态机并通过故障注入。
- [ ] 账户、市场、单笔、总库存、日损和速率限额均为默认拒绝。
- [ ] 订单、成交、余额、持仓、费用和结算可以自动对账。
- [ ] 全局 kill switch 与未知订单状态恢复经过演练。
- [ ] 首次 live canary 有独立审批、极小额度、明确观察期和回滚条件。

## 3. 首个正式 benchmark 的验收口径

首轮服务器 benchmark 应至少持续 24 小时，并输出：

- 地域、实例类型、内核、运行版本、数据源和订阅标的；
- 源事件数、接收数、重复数、断序数、解析错误、重连和静默窗口；
- P50/P95/P99 到达延迟，同时报告时钟偏差区间；
- 下单接口按 result/event → HTTP response/order ack → user stream → fill 分段，
  只读阶段使用平台允许的 sandbox/paper 或不产生真实订单的路径；
- 不带源时间戳的流只报告接收频率和更新间隔，不伪造单向延迟；
- 原始数据量、压缩比、磁盘写入、CPU、内存、网络与成本估算；
- 以实测 GB/day 分别外推 WAL、30/90 天 ClickHouse 和一年对象存储成本；
- 同一时段本地节点与云节点的对照，以及最差一小时而非只展示最佳区间。

建议的第一轮目标不是追求某个毫秒数字，而是证明：**数据不静默丢失、指标口径
可信、运行可恢复、成本可以外推。** 延迟 SLO 应在获得该数据后由人工批准。

## 4. 开发顺序建议

1. 固定双目标 venue 数据契约、采集器状态机和 24h benchmark runner。
2. 建立原始 WAL、Parquet/R2、manifest、质量检查和 canonical schema。
3. 用 24h/7d 样本确定 Bronze/Silver/Gold、ClickHouse DDL 和冻结数据集。
4. 开发 point-in-time research、确定性 replay、费用/结算和 paper OMS。
5. 全程强化 CI、安全、测试、观测、IaC、回滚和恢复等 DFX 能力。
6. 本地 L3 通过后才申请短期云节点，部署后先跑 `make verify-host`。
7. 根据 benchmark 和回放结果决定是否投入低延迟优化及 live execution。

任何步骤都不得因为“接口已接通”而跳过 G2/G3。

## 5. 2026-08-20 实施增量

已完成的本地工程闭环：

```text
现有公共源 NDJSON
  → RawEventEnvelope v1 校验
  → 64 MiB 默认分段 WAL
  → flush + fsync + 原子 seal
  → SHA-256/row/byte/time/source/stream manifest
  → 独立 verify
  → CanonicalMarketEvent v1
  → quality flags / quarantine
  → 输入与输出 checksum 绑定的 transform manifest
```

进程重启时，完整 NDJSON 行会被恢复并封存；未完成的尾部字节会原样进入 quarantine，
不会伪装成有效事件或静默丢弃。此闭环是本地实现证据，不等同于 G2 的 24h soak、磁盘满、
R2 回补或空 ClickHouse 恢复已经通过。Canonical v1 当前覆盖 Binance trade/BBO/depth 与
Polymarket trade/BBO/book/price change；未支持类型明确隔离，不能降级为含义不明的 Silver 行。

资源规格、三阶段采购上限和年度现金需求见
[`INFRASTRUCTURE_CAPACITY_AND_COST.md`](INFRASTRUCTURE_CAPACITY_AND_COST.md)。
详细优先级与 P2/P3 验收见 [`IMPLEMENTATION_ROADMAP.md`](IMPLEMENTATION_ROADMAP.md)。
