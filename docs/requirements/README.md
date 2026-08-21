# 需求文档索引

本目录是项目范围和开发门禁的事实来源。文档中的状态使用以下标记：

- `CONFIRMED`：有官方契约、实采数据或可重复证据。
- `INFERRED`：根据公开行为、命名、IP 或前端产物推断。
- `DECISION`：需要负责人明确选择。
- `BLOCKED`：缺少授权、凭据、样本或外部条件。

## 核心文档

| 文档 | 用途 | 更新触发 |
|---|---|---|
| [PRODUCT_REQUIREMENTS.md](PRODUCT_REQUIREMENTS.md) | 产品目标、系统能力、范围和验收标准 | 目标或范围变化 |
| [DEVELOPMENT_READINESS.md](DEVELOPMENT_READINESS.md) | 大规模开发、上云和实盘前门禁 | 每个里程碑前 |
| [OPEN_DECISIONS.md](OPEN_DECISIONS.md) | 人工决策及截止点 | 决策作出或失效 |
| [INFRASTRUCTURE_CAPACITY_AND_COST.md](INFRASTRUCTURE_CAPACITY_AND_COST.md) | 容量模型、租赁明细和阶段预算 | benchmark 或价格变化 |
| [IMPLEMENTATION_ROADMAP.md](IMPLEMENTATION_ROADMAP.md) | P0–P3 顺序、P2 数仓研究和完整 DFX | 阶段或验收变化 |
| [SYSTEM_ARCHITECTURE.md](../architecture/SYSTEM_ARCHITECTURE.md) | 目标架构和数据流 | 架构决策变化 |
| [DATA_FLOW_AND_LIFECYCLE.md](../architecture/DATA_FLOW_AND_LIFECYCLE.md) | 流式采集、分层管理、存储生命周期及回测/实盘用法 | 数据路径、保留期或消费模式变化 |
| [CANONICAL_DATA_MODEL.md](../architecture/CANONICAL_DATA_MODEL.md) | Silver v1、质量处置和 Raw lineage | parser 或质量语义变化 |
| [DATASET_AND_REPLAY.md](../architecture/DATASET_AND_REPLAY.md) | Parquet v1、Dataset Manifest v2 和确定性 Replay | 数据集或回放契约变化 |
| [INFERRED_ORIGINAL_TOPOLOGY.md](../architecture/INFERRED_ORIGINAL_TOPOLOGY.md) | 原项目可观察部署事实和置信度 | 新证据出现 |
| [DATA_SOURCE_STATUS.md](../../recon/DATA_SOURCE_STATUS.md) | 数据源、鉴权、连通性和表结构状态 | 数据源状态变化 |
| [PREDICTFUN_BENCHMARK_ONBOARDING.md](../runbooks/PREDICTFUN_BENCHMARK_ONBOARDING.md) | Predict.fun 测试网、API 申请和下单 benchmark 入门 | API/SDK 契约变化 |
| [DEPLOYMENT_VERIFICATION.md](../runbooks/DEPLOYMENT_VERIFICATION.md) | 本地优先门禁、发布候选和未来主机 smoke 操作手册 | 验证阈值或部署流程变化 |
| [MANUAL_ACTION_GUIDE.md](../runbooks/MANUAL_ACTION_GUIDE.md) | 人工负责人、操作、证据和阻塞条件 | 决策、账户或门禁变化 |

## 需求变更规则

1. 新功能先在需求文档中说明目的、边界和验收标准。
2. 推断必须标为 `INFERRED`，不能直接升级为目标要求。
3. 涉及交易、密钥、账户、KMS 或外部写操作的需求必须增加显式门禁。
4. 任何数据表必须能追溯到官方字段、实采 payload 或明确的派生公式。
5. 已实现需求应链接测试、数据样本或运行报告；口头完成不算验收。
