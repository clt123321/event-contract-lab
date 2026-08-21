# Exploration

探索区保存无法从最终白皮书替代的原始证据、观察记录和边界说明。这里的内容用于回答
“我们看到了什么”，不直接代表目标系统需求，更不代表已取得原项目源码或完整策略。

## `signalx/`

- [`EXPLORATION.md`](signalx/EXPLORATION.md)：公开前端与授权控制面的清洁室勘察；
- [`OBSERVABLE_SURFACE_FINDINGS_20260818.md`](signalx/OBSERVABLE_SURFACE_FINDINGS_20260818.md)：
  数据规模、运行链路、Agent 和 Winner/Tail Sweep 行为证据；
- [`INFERRED_ORIGINAL_TOPOLOGY.md`](signalx/INFERRED_ORIGINAL_TOPOLOGY.md)：AWS 东京/伦敦部署推断；
- [`DATA_SOURCE_STATUS.md`](signalx/DATA_SOURCE_STATUS.md)：数据源、连通性和缺口；
- [`api-surface.yaml`](signalx/api-surface.yaml)：可观察 API/事件面。

所有结论遵循：公开或授权只读、最少必要记录、事实与推断分离、不保存凭据和私有配置。

## `local-data/`

本地公共行情采集与验证报告，体积较大，不进入 Git。它们是技术可行性与网络诊断证据，
不是正式策略数据集，也不能单独支持收益结论。说明见 [`local-data/README.md`](local-data/README.md)。
