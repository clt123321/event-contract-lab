# 人工行动指南

版本：v0.2｜更新时间：2026-08-20｜适用阶段：云申请前至 G3

本清单只列不能由代码替人决定的事项。密钥、钱包、个人信息和付款资料不得写入 Git、issue
正文或普通日志；仓库只记录 secret reference、审批结果和证据哈希。

## 0. 执行顺序与当前负责人

| 顺序 | 谁来做 | 现在的动作 | 不做会阻塞什么 |
|---:|---|---|---|
| 1 | 项目负责人（用户） | 回复/跟进 Predict.fun 官方工单 | 三源正式 benchmark |
| 2 | 项目负责人 + 研究负责人 | 审核 Polymarket 3–5 个市场 | 正式 benchmark/dataset |
| 3 | 项目负责人 + 研究负责人 | 本地 24h 后批准 quality mask | 正式研究结论 |
| 4 | 项目负责人 | 审核本地 soak、IaC plan 和二源/三源起步 | 云申请 |
| 5 | billing/technical owner | 建 AWS 项目、角色、预算告警和销毁日 | 14 天云 benchmark |
| 6 | 法务/合规/资金/风控负责人 | G3 独立书面审批 | 任何真实下单 |

顺序 1–3 可并行准备，但只有 4 通过才启动 5；6 与当前数据阶段分离。

## A. 云申请前必须完成

### A1. 批准研究质量掩码

- [ ] 负责人：项目负责人 + 研究负责人。
- [ ] 输入：24h 本地 `quality.json`、各 flag 的样本行和研究目的。
- [ ] 逐项决定：`missing_source_timestamp`、`source_timestamp_after_receive`、
  `stale_event`、`one_sided_book`、`empty_book`、`sequence_gap`。
- [ ] 推荐：延迟研究保持 strict；仅使用 `available_at_ms` 的盘口状态研究可另建 mask，按用途
  允许 `missing_source_timestamp`，不要修改 strict-v1。
- [ ] 操作：复制为新的 `config/quality-mask.<purpose>-vN.json`，填写唯一 `mask_version`，PR 审批。
- [ ] 运行 `make dataset TRANSFORM_MANIFEST=<24h-transform-manifest> OUTPUT_DIR=<new-dataset-dir>`，
  核对 `input_rows = included_rows + excluded_rows`、`exclusion_counts` 和研究覆盖率。
- [ ] 用新目录再跑一次，确认 dataset ID 和 Parquet SHA-256 不变。
- [ ] 完成证据：批准人、日期、研究用途、样本报告 SHA-256、配置 commit。
- 阻塞影响：未批准时可以开发，但不能发布正式研究 dataset 或策略结论。

### A2. 冻结 Polymarket 3–5 个市场

- [ ] 负责人：项目负责人。
- [ ] 运行 `npm --prefix benchmark run discover:polymarket -- --query bitcoin --limit 10`，对
  `ethereum` 等目标各跑一次；保存候选的 question、slug、condition ID、每个 outcome/token ID、
  end time、规则、resolution source、order-book 状态和流动性证据。
- [ ] 排除临近结束、规则含糊、无订单簿或流动性不足的市场。
- [ ] 人工核对 token 与 outcome，禁止仅按数组位置猜 YES/NO。
- [ ] 将批准项写入 `config/market-universe.json`，附 evidence reference、reviewer 和 review time。
- [ ] 提交前运行 `make readiness` 和 `make verify-local`，确认市场数量门禁不再阻塞。
- [ ] 完成证据：3–5 个固定 market/token ID、规则指纹和审批 commit。
- 阻塞影响：不阻塞动态 smoke；阻塞正式 benchmark、跨 venue mapping 和正式 dataset。

### A3. Predict.fun 官方接入

- [ ] 负责人：项目负责人；外部依赖：Predict.fun 官方支持。
- [ ] Discord 工单明确申请 Testnet/read-only、目标 BTC/ETH 短周期、预期订阅频率和 benchmark
  用途；不申请主网写入。
- [ ] 保存工单编号、批准范围、限流、环境、到期时间和官方文档版本。
- [ ] 把官方回复整理成一张无 secret 的契约表：base URL、WS URL、auth header 名、
  market ID、channel、rate limit、heartbeat、sequence/reconnect 语义；模糊项继续追问，不自行猜。
- [ ] key 只进入本地/云 secret manager；仓库记录 secret reference 名称，不记录值。
- [ ] 提供最小脱敏 fixture，人工检查无账户、钱包、签名和 token。
- [ ] 完成证据：许可截图/工单引用、脱敏契约 fixture、固定 Testnet market ID。
- 阻塞影响：阻塞三源正式 benchmark；不阻塞 Binance + Polymarket 本地开发。

### A4. 决定两源还是三源启动云 benchmark

- [ ] 负责人：项目负责人。
- [ ] 截止：本地 24h soak、部署 artifact 和 IaC 评审完成时。
- [ ] 推荐：Predict.fun 权限已到则三源一起开始；若外部等待超过项目允许窗口，书面批准先做
  Binance + Polymarket，并保留 Predict.fun 补测预算。
- [ ] 完成证据：决策日期、理由、目标源、市场版本、预算拆分和补测触发条件。
- 阻塞影响：未决定时不应开始消耗 14 天云租期。

### A5. 审核本地上云候选

- [ ] 负责人：项目负责人 + 技术负责人。
- [ ] 输入：`make verify-release` 的 passed 报告、24h soak 报告、故障演练记录、
  Terraform plan、artifact 哈希和回滚/销毁 runbook。
- [ ] 逐项确认：live disabled；Parquet/Dataset/Replay 通过；磁盘/时钟/DNS 预检；
  无 secret；实例和磁盘均能按 IaC 销毁。
- [ ] 完成证据：发布 commit、report SHA-256、plan SHA-256、审核人/日期和 GO/NO-GO。
- 阻塞影响：NO-GO 时不申请云资源，但可继续本地修复。

## B. 申请云资源时完成

### B1. AWS 项目与预算

- [ ] 创建独立项目账号/成本中心，不使用个人长期管理员凭据部署。
- [ ] 指定 billing owner、技术 owner、告警接收人和替补联系人。
- [ ] 建立 `$100` 预警与 `$150` 强告警；确认 14 天后停止/销毁负责人。
- [ ] 不购买 Savings Plan/Reserved Instance；不在实测前配置多地域或 4 TB 热盘。
- [ ] 创建最小权限 deploy role；只授权明确 region、实例、EBS、日志和预算读取能力。
- [ ] 完成证据：账号别名、role ARN、预算规则 ID、接收人确认、销毁日期；均不得包含 secret。

### B2. 地域与合规边界

- [ ] 确认东京节点只用于只读测量，不能据此推定 Polymarket/Predict.fun 交易资格。
- [ ] 保存平台条款/地域检查日期和负责人；禁止 VPN/geoblock 规避方案。
- [ ] 完成证据：书面只读范围和 NO-LIVE 声明。

## C. 实盘前独立审批（当前不执行）

- [ ] 明确账户、资金和税务主体。
- [ ] 法律/合规确认地域与平台资格。
- [ ] 审批钱包、token allowance、密钥轮换和应急吊销流程。
- [ ] 审批 `$0 → $100 → $300 → $1,000 → $3,000` 各风险级别；每级单独复审。
- [ ] 确认 OMS、账本、对账、unknown state、kill switch 和结算演练证据。
- [ ] 任一项缺失即保持 `live_enabled=false`。

## D. 每周人工检查模板

```text
日期：
负责人：
本周批准/拒绝事项：
引用的报告路径与 SHA-256：
市场配置 commit：
quality mask commit：
外部工单状态：
预算累计/预测：
新风险或条款变化：
下周需要谁在何时做什么：
```

## E. 交给项目仓库的最小信息

人工操作完成后，只需提交以下无敏感信息：

- 市场：market/condition/token ID、rules/evidence reference、reviewer、review time；
- 外部权限：ticket ID、environment、scope、rate limit、expiry、secret reference 名；
- 数据审批：quality mask 版本、用途、样本报告 hash、批准人/日期；
- 云资源：account alias、region、role ARN、budget rule ID、负责人、销毁日；
- 门禁：commit、report/plan/artifact hash、GO/NO-GO、理由。

任何 key/token/secret value、seed phrase、签名、完整账号或付款信息都不得提交。
