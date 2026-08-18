# Predict.fun benchmark 入门与安全边界

版本：v0.1｜核对日期：2026-08-18｜状态：Read-only approved / Testnet write pending confirmation

## 1. 什么是目标 venue

`venue` 是订单最终提交、确认、成交和结算的交易场所，不等同于所有数据源。
当前建议的职责划分是：

| 角色 | 系统 | 当前用途 |
|---|---|---|
| 目标 execution venue | Predict.fun | orderbook、订单 ack、wallet event、fill benchmark |
| 参考行情源 | Binance | 标的现货/盘口与价格触发 |
| 预言机/结算参考 | Chainlink | 待确认具体 feed 与消息契约 |
| 研究对照 | Polymarket、Kalshi 等 | 跨 venue 概率和研究，不默认执行 |

选择 Predict.fun 为目标 venue，意味着 OMS 首个 adapter 和端到端延迟口径围绕它开发；
不代表马上使用主网资金，也不排除以后增加其他 venue。

## 2. 官方环境与鉴权

| 环境 | REST | API key | 钱包/JWT | 资金风险 |
|---|---|---|---|---|
| BNB Testnet | `https://api-testnet.predict.fun/` | 不需要；官方限速 240 req/min | 个性化/订单操作仍需钱包签名认证 | 测试资产，不应有主网价值 |
| BNB Mainnet | `https://api.predict.fun/` | 需要；Discord support ticket 申请 | 下单、订单查询需要 JWT | 真实订单、授权和资金 |
| Mainnet WebSocket | `wss://ws.predict.fun/ws` | 握手需要 API key | wallet events topic 还需要 JWT | 只读市场流低风险；账户流敏感 |

主网认证流程是：`GET /v1/auth/message` → 钱包签名 → `POST /v1/auth` 获取 JWT；
主网请求同时携带 `x-api-key` 和 `Authorization: Bearer <JWT>`。JWT、API key、助记词、
Privy/private key 都不得进入 Git、聊天、命令行参数或普通日志。

官方支持 EOA 和由网页自动创建的 Predict Account/Smart Wallet。探索阶段使用独立 Testnet
EOA，避免导出现有网页账户的 Privy key；主网账户形态等安全评审后再决定。

## 3. “对着下单接口测”的四种方式

### A. Testnet 实际下单链路（建议首选，执行前确认）

向测试网订单接口提交签名订单，测量 request、HTTP 201、order ID/hash、wallet event、
撮合/成交和取消。它最接近真实协议，又不应使用真实资金。

能证明：客户端签名、REST ack、账户事件、状态机和测试网撮合路径。

不能证明：主网公网路由、主网撮合负载、真实资金结算和主网 P99。

### B. 本地 dry-run（当前批准）

完成市场读取、金额计算、EIP-712 构造、签名和序列化，在 HTTP 提交前停止；或者提交到
我们自己的 mock server。

能证明：客户端处理耗时、payload 和状态机；不能证明平台 order ack/fill。

### C. Mainnet post-only 限价单并撤单（尚未批准）

提交不会立即吃单的 post-only LIMIT，收到 accepted/wallet event 后撤单。它可以测真实
主网 ack 和账户流，但订单在撤单前对市场可见，价格变化、参数错误或平台语义变化仍可能
导致成交；撤单也可能是链上交易。

前置条件：平台书面确认允许 benchmark、账户/地域资格、专用小额钱包、金额/频率上限、
post-only 行为验证、自动撤单、未知状态恢复和 kill switch。

### D. Mainnet 极小真实成交 canary（尚未批准）

使用极小 marketable/FOK 订单，测到真实 fill 与结算。这是最接近生产的方式，也是唯一
能验证完整主网成交路径的方式。

它会产生真实头寸、费用、滑点和损失，必须通过 G3 风控/合规门禁并由风险负责人单独批准。
不得把它当作高频压测。

反复发送无效订单、故意触发 401/400 或在主网创建后快速撤单，不应被当作无风险替代品：
处理路径不同，也可能触发限流、风控或违反平台政策。

## 4. 推荐上手顺序

### Step 1：向官方确认边界

加入官方 Discord，在 support ticket 中询问 API key 和 benchmark 许可。可使用：

```text
Hello Predict support,

We are building a research and latency benchmark for Predict.fun. We will begin
on BNB Testnet and keep Mainnet read-only until you confirm the permitted scope.

Could you please confirm:
1. the current Testnet REST and WebSocket endpoints;
2. how to obtain Testnet assets and a wallet JWT;
3. whether repeated Testnet create/cancel tests are permitted and the rate limit;
4. whether Mainnet post-only create/cancel latency benchmarking is permitted;
5. the recommended request rate, order size, market selection and identification;
6. whether an API key can be issued for Mainnet read-only market/WebSocket access.

We will not share keys, private keys or JWTs in the ticket or repository.
```

### Step 2：建立隔离身份

- 新建只用于 Testnet 的 EOA；不要使用持有主网资产的钱包；
- 从官方支持确认 faucet/测试资产来源，不相信私信链接；
- secret 只存本机 keychain 或云 Secrets Manager；
- 默认配置 `LIVE_EXECUTION=false`，代码中还需要第二个显式开关和环境白名单。

### Step 3：先只读

- `GET /markets` 发现一个开放的测试市场；
- 拉取 `/markets/{marketId}/orderbook`；
- 订阅 orderbook、trading status、market status；
- 验证 heartbeat、时间戳、重连、rate limit 和 payload schema。

### Step 4：Testnet 订单闭环

- 获取 auth message、签名并换 JWT；
- 启动 wallet events 订阅；
- 先运行本地 dry-run，人工核对 maker、signer、tokenId、feeRateBps、金额和 chain ID；
- 提交一个最小 LIMIT 测试单，记录 HTTP ack/order ID/hash；
- 观察 `orderAccepted`、fill/cancel 事件并与 REST 查询对账；
- 注入超时、重复提交、断线和未知状态，验证幂等与恢复。

### Step 5：14 天东京 benchmark

前 7 天 `c7i.large`、后 7 天 `c7i.xlarge`。市场数据可连续采集；订单测试严格遵循官方
确认的频率，不需要每条市场消息都下单。每个订单样本记录：

```text
trigger_recv_mono_ns
build_start/end_ns
sign_start/end_ns
http_send_ns
http_headers/complete_ns
order_id/order_hash
wallet_accepted_ns
first_fill_ns/final_fill_ns
cancel_requested/confirmed_ns
outcome/status/error
```

## 5. 当前授权状态

- 已批准：14 天 benchmark 的 $150 预算、本地 dry-run、公开/授权只读市场数据；
- 待确认：官方回复测试网 create/cancel 许可与频率后，再执行测试网订单闭环；
- 未批准：导出真实资金账户私钥、设置主网 token approvals、主网订单 create/cancel；
- 未批准：主网真实成交、自动策略、资金划转与长期 live agent。

## 6. 官方资料

- [Predict API FAQ](https://dev.predict.fun/)
- [Predict API authentication](https://dev.predict.fun/ts-how-to-authenticate-your-api-requests-663127m0)
- [Predict WebSocket](https://dev.predict.fun/general-information-1915499m0)
- [Subscription topics](https://dev.predict.fun/subscription-topics-1915507m0)
- [Create an order](https://dev.predict.fun/create-an-order-32534694e0)
- [Official TypeScript SDK](https://github.com/PredictDotFun/sdk)
