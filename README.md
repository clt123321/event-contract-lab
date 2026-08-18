# event-contract-lab

事件合约量化系统的清洁室研究、数据接入、延迟基准和工程复建仓库。

本仓库根据公开协议、授权访问下的可观察行为和自行采集的数据构建，不复制目标
系统的私有源码、密钥、生产配置或策略参数。当前重点是先验证数据契约和时间口径，
再建设回放、策略、执行、风控和控制面。

## 当前状态

- Binance 与 Polymarket 公共行情已在本地接通。
- 已建立 NDJSON 原始事件结构、时钟探针、网络诊断和延迟汇总。
- 当前网络的系统 DNS 存在污染，采集器已支持项目内 DoH。
- 已恢复 SignalX 可观察的控制面/API 边界和推断部署拓扑。
- 大部分公共数据层、回放层和控制面骨架可以开始开发。
- 大规模服务器部署和实盘执行仍受开发门禁约束。

## 文档入口

- [需求文档索引](docs/requirements/README.md)
- [产品与系统需求](docs/requirements/PRODUCT_REQUIREMENTS.md)
- [开发与部署准备度](docs/requirements/DEVELOPMENT_READINESS.md)
- [待人工决策清单](docs/requirements/OPEN_DECISIONS.md)
- [目标系统架构](docs/architecture/SYSTEM_ARCHITECTURE.md)
- [原项目推断拓扑](docs/architecture/INFERRED_ORIGINAL_TOPOLOGY.md)
- [数据源与连通性状态](recon/DATA_SOURCE_STATUS.md)
- [清洁室勘察记录](recon/EXPLORATION.md)
- [公共接口基准工具](benchmark/README.md)

## 快速验证

```bash
cd benchmark
npm install
npm test
npm run diagnose:network -- --dns-only
npm run probe:clock -- --samples 5 --dns doh
```

## 仓库边界

默认允许：公开行情、只读发现、原始数据建模、质量检测、回放、paper trading、
控制台与 Agent 框架。

默认禁止：把密钥提交到 Git、未经批准下单、绕过平台地域限制、复制私有源码、
将推断字段冒充官方契约、在缺少风险门禁时启用实盘。
