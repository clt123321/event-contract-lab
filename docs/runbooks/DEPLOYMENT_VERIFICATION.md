# 本地优先与部署后验证手册

版本：v0.2｜更新时间：2026-08-20｜适用范围：只读数据链路

## 1. 原则与次序

云服务器申请后移到本地发布候选通过之后。代码在本地和服务器上使用同一个验证入口、
同一份阈值配置和同一种 `report.json`，避免上线后依赖临时 SSH 命令判断系统是否正常。

```text
L0 契约/fixture
→ L1 本地单元与静态检查
→ L2 本地 Raw→WAL→verify 和恢复/损坏测试
→ L3 clean commit 发布候选验收
→ 申请短期云资源
→ H0 主机预检
→ H1 30 秒只读公共源 smoke
→ H2 24h/7d/14d benchmark
```

申请服务器之前至少需要一份状态为 `passed`、Git 为 clean 的 L3 报告。服务器仍只是
验证环境，不等于 G3 实盘授权。

## 2. 本地开发验收

日常开发运行：

```bash
make verify-local
```

它会检查 Node/磁盘、Git 状态、live safety、Rust fmt/Clippy/test、Node test，并构建
`wal-cli`、`normalize-cli`、`dataset-cli` 和 `replay-cli`。最后用 synthetic fixture 跑通：

```text
Raw → WAL/segment manifest/checksum verify
    → Canonical Silver/quality/quarantine/transform manifest
    → strict quality mask/ZSTD Parquet/Dataset Manifest v2
    → point-in-time replay/Replay Manifest v1
```

工作区有未提交修改时报告为 `warning`，便于开发中持续运行。

准备发布候选时：

```bash
make verify-release
```

该命令增加 clean Git 门禁；任何错误检查失败都会以非零状态退出。输出默认位于
`data/verification/<run-id>/`，其中包括：

- `report.json`：机器可读总结果、阈值、commit、主机信息和每项检查；
- `logs/*.stdout.log`、`logs/*.stderr.log`：每一步的原始诊断；
- `wal/`、`silver/`、`dataset/`、`replay/`：本次验证的分层 artifact 与 manifest。

正式发布报告不提交 Git；需要保留时将整个目录作为 CI artifact 或运维证据归档。

## 3. 未来服务器上线后验证

部署代码和依赖后，从仓库根目录运行：

```bash
make verify-host
```

默认执行 30 秒只读 smoke：

1. 主机运行时、磁盘、Git commit 和 live safety；
2. DNS、DoH、TLS、HTTP 和 WebSocket 分层诊断；
3. Binance server time 时钟偏差探针；
4. Binance + Polymarket 公共行情短采集；
5. 延迟、重连、解析错误、断序 summary；
6. 实采 NDJSON 导入 WAL 并核对 checksum/行数。
7. 同一份实采数据转为 Canonical Silver，并检查行数守恒、lineage 和 quarantine 比例。
8. 使用 strict mask 冻结非空 Parquet dataset，再完整 replay，核对 dataset/output hash 和行数。

调整时长或网络参数不需要改代码：

```bash
make verify-host VERIFY_ARGS="--duration 120 --dns doh --symbol ETHUSDT --polymarket-query ethereum"
```

`host-smoke` 可以动态发现 Polymarket 市场以判断网络与解析器是否工作，但报告固定给出
`market_selection.formal = false` 警告。24h 正式 benchmark 必须使用已经版本化并人工
批准的 market/token ID，不能把动态热门市场 smoke 当作正式样本。

该脚本不接受 API key、钱包或账户参数，且没有下单能力。

## 4. 快速定位与调整闭环

| 首个失败检查 | 优先检查 | 允许的快速调整 |
|---|---|---|
| `runtime.*` | Node 版本、磁盘空间 | 升级运行时；扩容/清理专用数据盘 |
| `network.*.doh` | 出口、443、防火墙、DoH | 修正安全组/出口；不关闭 TLS 校验 |
| `network.*.protocol` | DNS、TLS、HTTP/WS 路由 | 切换已验证 DNS；检查代理和平台状态 |
| `clock.*` | chrony/云时间服务 | 修复同步后重新完整采样，不手改报告 |
| `capture.source.*` | 订阅 ID、心跳、市场活跃度 | 使用已批准 ID；延长 smoke；查看 capture 日志 |
| `capture.parse_errors` | schema drift、源 payload | 保存脱敏 fixture、隔离数据、升级 parser |
| `wal.*` | 权限、空间、锁、checksum | 停止第二写者；恢复磁盘；从 manifest 复核 |
| `dataset.*` | quality mask、输入/Parquet hash、非空行数 | 保留原 artifact；更正上游或新建 mask 版本；不手改 manifest |
| `replay.*` | dataset/config/commit 绑定、时间序、output hash | 用全新目录重放；对比 manifest；不覆盖旧证据 |

每次调整都创建新的 run directory，旧报告不覆盖。比较前后报告的 commit、参数和主机信息；
禁止直接编辑失败报告改成通过。

可以直接比较两次运行的检查变化和步骤耗时：

```bash
make compare-verify \
  BEFORE=data/verification/<before>/report.json \
  AFTER=data/verification/<after>/report.json
```

输出会分别列出 `improved`、`regressed`、`unresolved`，并指出验证阈值或市场配置 hash
是否发生变化，防止把“降低门槛”误当作系统修复。

## 5. 服务器申请门禁

满足以下条件后再申请 14 天短期节点：

- `make verify-release` 状态为 `passed`；
- 本地故障/恢复测试覆盖重启、partial tail、checksum 篡改、Parquet 篡改和双写者冲突；
- 本地 24h soak 的断流、质量、吞吐和磁盘外推报告已人工审核；
- 目标市场配置、验证阈值、部署 artifact/commit 已冻结；
- Terraform/部署脚本可以重复创建与销毁，不依赖手工改机器；
- AWS 账号、最小权限角色、预算告警接收人和 `$150` 硬上限明确；
- 上线后首先运行 `make verify-host`，失败时不进入长时间 benchmark。
