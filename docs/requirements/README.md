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
| [SYSTEM_ARCHITECTURE.md](../architecture/SYSTEM_ARCHITECTURE.md) | 目标架构和数据流 | 架构决策变化 |
| [INFERRED_ORIGINAL_TOPOLOGY.md](../architecture/INFERRED_ORIGINAL_TOPOLOGY.md) | 原项目可观察部署事实和置信度 | 新证据出现 |
| [DATA_SOURCE_STATUS.md](../../recon/DATA_SOURCE_STATUS.md) | 数据源、鉴权、连通性和表结构状态 | 数据源状态变化 |

## 需求变更规则

1. 新功能先在需求文档中说明目的、边界和验收标准。
2. 推断必须标为 `INFERRED`，不能直接升级为目标要求。
3. 涉及交易、密钥、账户、KMS 或外部写操作的需求必须增加显式门禁。
4. 任何数据表必须能追溯到官方字段、实采 payload 或明确的派生公式。
5. 已实现需求应链接测试、数据样本或运行报告；口头完成不算验收。
