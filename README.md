# event-contract-lab

事件合约量化系统的清洁室研究、数据接入、延迟基准和工程复建仓库。

本仓库根据公开协议、授权访问下的可观察行为和自行采集的数据构建，不复制目标
系统的私有源码、密钥、生产配置或策略参数。当前重点是先验证数据契约和时间口径，
再建设回放、策略、执行、风控和控制面。

## 当前状态

- Binance 与 Polymarket Gamma/Data/CLOB/Market WS 公共行情已在本地接通。
- 已建立 Rust workspace、版本化 schema、固定市场配置和 CI 安全门。
- 已建立 NDJSON 原始事件结构、分段 WAL、重启恢复、SHA-256 manifest、时钟探针、
  网络诊断和延迟汇总。
- 当前网络的系统 DNS 存在污染，采集器已支持项目内 DoH。
- 已恢复 SignalX 可观察的控制面/API 边界和推断部署拓扑。
- Predict.fun 与 Polymarket 已确认为双目标 venue，第一期仅推进公开/授权只读数据链路。
- Binance 固定 BTCUSDT/ETHUSDT；Predict.fun 与 Polymarket 的正式市场 ID 仍等待人工/外部冻结。
- Raw → WAL → seal/verify 已可本地运行；Parquet/R2、canonical、ClickHouse 和 replay 尚未完成。
- 大规模服务器部署和实盘执行仍受开发门禁约束。

## 文档入口

- [需求文档索引](docs/requirements/README.md)
- [产品与系统需求](docs/requirements/PRODUCT_REQUIREMENTS.md)
- [开发与部署准备度](docs/requirements/DEVELOPMENT_READINESS.md)
- [P0–P3 实施路线图与 DFX](docs/requirements/IMPLEMENTATION_ROADMAP.md)
- [待人工决策清单](docs/requirements/OPEN_DECISIONS.md)
- [基础设施容量与成本](docs/requirements/INFRASTRUCTURE_CAPACITY_AND_COST.md)
- [目标系统架构](docs/architecture/SYSTEM_ARCHITECTURE.md)
- [原项目推断拓扑](docs/architecture/INFERRED_ORIGINAL_TOPOLOGY.md)
- [数据源与连通性状态](recon/DATA_SOURCE_STATUS.md)
- [清洁室勘察记录](recon/EXPLORATION.md)
- [SignalX 可观察面深挖报告](recon/OBSERVABLE_SURFACE_FINDINGS_20260818.md)
- [公共接口基准工具](benchmark/README.md)

## 快速验证

```bash
make bootstrap
make check
make readiness  # 输出仍需项目负责人/外部平台完成的阶段门禁
make verify-local  # 开发中完整本地验收；dirty Git 只告警
make verify-release  # 云申请/发布前验收；要求 clean Git

# 将 synthetic fixture 导入分段 WAL，并逐 segment 校验行数、字节数和 SHA-256
cargo run --locked -p wal-cli -- import \
  --input fixtures/raw/sample-events.v1.ndjson \
  --wal-dir data/wal \
  --max-segment-bytes 512
cargo run --locked -p wal-cli -- verify --wal-dir data/wal
```

公共源实采仍从现有 Node 工具进入；输出可直接交给同一个 WAL：

```bash
cd benchmark
npm run collect:binance -- --symbol BTCUSDT --duration-seconds 60
npm run snapshot:polymarket -- --query bitcoin

cd ..
make wal-import INPUT=benchmark/data/raw/<capture>.ndjson
```

`config/market-universe.json` 是首期范围的版本化事实源。CI 会拒绝启用 venue write；
本仓库当前没有真实下单实现。

云资源申请已后移：先尽量完成本地开发、故障恢复和发布候选验收。未来服务器部署后复用
同一验证器运行 `make verify-host`，生成网络、时钟、公共行情、summary 和 WAL 证据包。
详见[部署验证手册](docs/runbooks/DEPLOYMENT_VERIFICATION.md)。

## Monorepo 布局

| 路径 | 当前职责 |
|---|---|
| `benchmark/`, `collectors/` | 已跑通的公共源探针与后续 source adapter |
| `crates/event-contracts` | Raw、market mapping、segment/dataset manifest 契约 |
| `crates/collector-core` | 单写者分段 WAL、崩溃尾部隔离、seal/verify |
| `apps/wal-cli` | NDJSON 导入、恢复和校验命令 |
| `schemas/`, `config/` | 不可重解释的 schema 与人工审批市场范围 |
| `replay/`, `execution/`, `control/`, `research/` | P2/P3 模块边界；尚未宣称已实现 |
| `infra/` | G2 IaC 边界；当前不会创建云资源 |

## 仓库边界

默认允许：公开行情、只读发现、原始数据建模、质量检测、回放、paper trading、
控制台与 Agent 框架。

默认禁止：把密钥提交到 Git、未经批准下单、绕过平台地域限制、复制私有源码、
将推断字段冒充官方契约、在缺少风险门禁时启用实盘。
