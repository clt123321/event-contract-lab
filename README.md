# event-contract-lab

事件合约量化的策略研究与清洁室探索仓库。

当前仓库刻意保持精简：先判断市场哪里可能低效、策略为什么可能有效、需要什么最少数据，
再决定是否重建采集、回放和执行代码。此前完成的 Rust/Node 数据链路原型已证明公共行情、
WAL、Parquet、Dataset Manifest 和 Replay 的技术可行性；v0.2 后不再把过渡实现作为长期资产，
需要时可从 Git 历史恢复或按新的策略需求重建。

## 入口

- [事件合约系统化交易白皮书 v0.2](docs/WHITEPAPER.md)：市场结构、五类策略、验证方法、Demo 结论与路线图；
- [探索区说明](exploration/README.md)：SignalX Demo 证据、数据源调查和本地采集证据边界；
- [基础设施边界](infra/README.md)：何时才值得重新建设代码或申请云资源。

## 目录

```text
event-contract-lab/
├── README.md
├── docs/
│   └── WHITEPAPER.md
├── exploration/
│   ├── README.md
│   ├── signalx/          # 授权只读观察、公开接口与清洁室推断
│   └── local-data/       # 本地实采证据；大文件不进入 Git
└── infra/
    └── README.md         # 基础设施门禁；当前不部署云资源
```

## 仓库边界

允许：公开资料、公开 API、授权只读观察、独立策略研究、Paper/Shadow 设计。

禁止：复制私有源码或策略参数、提交密钥、未经批准下单、规避平台地域/账户限制、将推断冒充事实。

Live execution 保持关闭。
