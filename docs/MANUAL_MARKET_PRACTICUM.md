# 事件合约盘口实习行动指南

版本：v0.1｜更新日期：2026-09-08｜阶段：手工认知与策略发现

> 目标不是靠几笔交易赚钱，而是用最小成本理解盘口、规则、成交和结算，并把直觉转成可证伪的策略假设与数据模型。

## 1. 为什么现在先“玩几局”

当前最合理的顺序是：

```text
观察真实盘口
→ 手工经历订单生命周期
→ 记录困惑、摩擦与可能的低效
→ 写成策略假设
→ 由策略反推最小数据模型
→ 再建设采集、回放和执行系统
```

手工交易能回答“界面和订单到底如何工作”，不能单独证明策略有正期望。三五次输赢没有统计意义；
真正的交付物是观察记录、字段定义、策略草案和下一轮实验设计。

## 2. 先看懂什么是盘口

Polymarket 使用中央限价订单簿（CLOB）。`bid` 是买方愿付的最高价，`ask` 是卖方愿收的最低价，
两者之差是 spread。界面展示价可能是 bid/ask 中点或最近成交价，不等于此刻能成交的价格；买入通常看
ask，卖出通常看 bid。所谓 market order 本质上也是一个会立即吃掉现有挂单的限价单。
[官方盘口说明](https://docs.polymarket.com/concepts/prices-orderbook)

每次看市场，必须同时读下面四层：

| 层 | 要看什么 | 为什么重要 |
|---|---|---|
| 合约 | 问题、YES/NO、截止时间、resolution source、例外条款 | 标题相似不代表同一个合约 |
| 盘口 | best bid、best ask、spread、各档数量、tick size | 决定真实可成交价格与容量 |
| 成交 | 最近成交、成交方向、频率、价格跳变 | 区分报价变化和真实交易 |
| 生命周期 | submit、ack、live、partial fill、fill、cancel、settlement | 决定回测必须模拟哪些状态 |

规则高于标题。市场结算时，获胜 token 可兑付 1，失败 token 归零；Polymarket 的具体 resolution source、
end date 和 edge cases 定义在市场规则中。[官方结算说明](https://docs.polymarket.com/concepts/resolution)

## 3. 安全边界：把“学费”先封顶

按以下顺序逐级进行，前一级未完成不得进入下一级：

1. **只读观察**：不登录、不充值、不下单；
2. **Paper 手填**：看到盘口后在记录表中模拟订单；
3. **最小真实体验**：仅在资格、地域、账户和预算全部确认后，由项目负责人单独批准；
4. **自动交易**：本阶段禁止。

真实体验的默认门禁：

- 调用官方 geoblock 检查，结果允许交易；不得使用 VPN、云区或代理规避限制；
- 阅读并接受平台条款，确认账户主体和所在地允许操作；
- 预先写下不可追加的总学费上限，建议只使用即使全部损失也不影响生活的金额；
- 不使用借款、杠杆或自动追单；不因上一笔亏损临时增加金额；
- 密钥、助记词和 API credential 不进入仓库、截图、聊天或实验记录；
- 只选规则清楚、盘口有深度、可以解释最坏损失的合约；
- 任何规则疑义、订单状态 `unknown`、异常扣款或地域提示都立即停止。

Polymarket 要求下单前检查请求 IP 的地域资格；受限请求会被拒绝。
[官方地域限制与检查接口](https://docs.polymarket.com/api-reference/geoblock)

## 4. 五局实习

这里的“一局”是一段完整观察或订单实验，不等于一定要下注。

### 第 0 局：读懂一张市场卡片

选择一个高流动性、二元、规则简单的市场，用 20–30 分钟只观察。

记录：

- market URL、market/condition/token ID；
- 完整问题、规则文本、截止时间、resolution source；
- YES 和 NO 的 best bid/ask、spread、前三至五档数量；
- tick size、minimum order size、是否启用费用；
- 页面展示概率与真实买入/卖出价格的差异；
- 如果现在买入，最坏损失、最大兑付和盈亏平衡概率。

通过条件：能不用“涨跌”这种模糊词，精确说出自己买的是什么法律/经济现金流。

### 第 1 局：Paper Taker

挑一个你能独立估计概率的市场，在纸面上模拟立即成交：

1. 在看答案或后续新闻前写下概率区间，例如 `p ∈ [0.57, 0.63]`；
2. 记录当时 best ask、目标数量和可见深度；
3. 按吃单路径计算加权成交价，而不是使用中间价；
4. 加上该市场实际 fee schedule；
5. 写下“为什么对手愿意在这个价格卖给我”；
6. 15 分钟、1 小时和事件结束时复查。

当前 Polymarket 费用按市场配置决定，部分类型对 taker 收费、maker 不收费；研究时应读取市场的
`feeSchedule`，不能把“全平台零手续费”写死。[官方费用说明](https://docs.polymarket.com/trading/fees)

### 第 2 局：Paper Maker

在同一市场模拟一笔真正愿意成交的限价挂单：

- 选择价格、数量、最长等待时间和取消条件；
- 每当 best bid/ask 变化，记录你的报价相对盘口的位置；
- 判断是否可能成交、部分成交，成交后是否立刻处于不利价格；
- 记录取消决定与“取消发出时订单可能已经成交”的风险。

这一局要形成两个直觉：排队优先级会影响成交，`没有成交` 也不是零成本，因为它可能意味着只有在
价格即将对你不利时才被挑中。官方订单生命周期区分 resting、matched、delayed、unmatched 以及链上
确认状态。[官方订单生命周期](https://docs.polymarket.com/concepts/order-lifecycle)

### 第 3 局：跨市场结构检查

选择一组有关联的市场，只做 Paper：

- 同一二元事件的 YES/NO；
- 一组互斥结果；或
- 一个事件与其子事件。

使用**可成交 bid/ask 和目标数量**检查概率约束，不使用页面展示价。把费用、不同结算条款、不同截止
时间和多腿不同步列为显式成本。发现价格和不等式冲突只算“候选异常”，不是自动等于套利。

### 第 4 局：最小真实订单，可选

只有在第 0–3 局记录完整、地域检查通过、负责人给出明确金额上限后才进行：

1. 选择平台允许的最小或接近最小数量；
2. 下单前截取规则和盘口，写下最大损失；
3. 使用限价控制最坏成交价；
4. 记录本地 submit 时间、页面/API ack、成交或取消时间、成交均价和费用；
5. 不补仓，不因为“差一点成交”追价；
6. 直到平仓或结算，对余额、持仓、成交和费用做一次人工对账。

这一局不是收益测试，而是订单状态机和账本需求访谈。若所在地区不允许交易，则以 Testnet 或 Paper
完整替代，不影响策略研究进度。

## 5. 每局必须填写的记录

```yaml
session_id: manual-YYYYMMDD-NN
observer: human-id
venue: polymarket
observed_at: ISO-8601-with-timezone

market:
  url: ""
  market_id: ""
  condition_id: ""
  yes_token_id: ""
  no_token_id: ""
  question: ""
  rules_snapshot_or_hash: ""
  end_time: ""
  resolution_source: ""
  tick_size: ""
  min_order_size: ""
  fee_schedule: ""

book_before:
  yes_best_bid: ""
  yes_best_ask: ""
  no_best_bid: ""
  no_best_ask: ""
  depth_for_target_size: ""
  displayed_price: ""

decision:
  estimated_probability_range: ""
  evidence_available_at_the_time: []
  expected_edge_before_cost: ""
  expected_edge_after_cost: ""
  why_counterparty_might_trade: ""
  invalidation_condition: ""

order:
  mode: observe|paper|live
  side: buy|sell
  outcome: yes|no
  type: limit
  limit_price: ""
  quantity: ""
  submitted_at: ""
  acknowledged_at: ""
  status_events: []
  average_fill_price: ""
  filled_quantity: ""
  fee: ""

review:
  horizons: []
  final_resolution: ""
  pnl: ""
  surprise: ""
  possible_strategy_hypothesis: ""
  missing_data: []
```

不要删除失败记录，也不要事后改写 `estimated_probability_range`。如果只能记一个时间，记数据实际到达
并被人看到的时间，而不是后来查询到的源时间。

## 6. 从体验反推策略

每完成一局，只允许提出这种形式的策略雏形：

```text
在【特定市场/状态】下，
由于【可解释的参与者行为或制度摩擦】，
当【当时可观察信号】出现时，
【可成交 bid/ask】在【时间尺度】内存在【方向和幅度】的偏差；
若【证伪指标】出现，则放弃该假设。
```

坏例子：“BTC 涨了就买 YES。”

较好例子：“在 BTC 短周期阈值市场、剩余 5–30 分钟且盘口深度超过目标数量时，Binance 现货在 1 秒内
越过预先定义的 shock 阈值后，事件合约 ask 的条件响应慢于 500 ms；若按到单时 ask、费用和滑点计算
的样本外净 edge 不为正，则否定。”

## 7. 从策略反推最小数据模型

前五局结束后，先冻结下面九类对象，不急着设计数十张表：

| 对象 | 核心字段 | 哪种体验会验证它必要 |
|---|---|---|
| `MarketContract` | 规则、时间、resolution source、状态 | 第 0/3/4 局 |
| `OutcomeInstrument` | YES/NO token、互斥/嵌套关系 | 第 0/3 局 |
| `BookSnapshot` | bid/ask 档位、数量、source/receive time | 全部策略 |
| `PublicTrade` | 价格、数量、方向、时间 | Lead-lag/微观结构 |
| `ReferenceMarketEvent` | Binance trade/BBO、时间 | Fair Probability/Lead-lag |
| `DecisionObservation` | 当时证据、概率区间、理由、版本 | 防止事后解释 |
| `OrderIntent/Event` | 意图、submit/ack/live/cancel/status | 第 2/4 局 |
| `Fill/Position/LedgerEntry` | 成交、费用、持仓、现金变化 | 第 4 局与 Paper OMS |
| `Resolution` | 结果、提议/争议/最终时间 | 所有持有到期策略 |

所有事件再由统一 envelope 包裹：`source`、`stream`、`market identity`、源时间、收件时间、sequence、
session 和原始 payload。只有当某个策略或失败模式要求时才增加字段。

## 8. 实习结束的交付与 Gate

完成标准不是“五局赢了几局”，而是：

- 至少 3 个市场卡片记录，其中包括短周期、清晰规则和关联市场三种结构；
- 至少 2 笔 Paper Taker、2 笔 Paper Maker、1 次结构扫描；
- 每笔都有决策前概率、真实 bid/ask、目标深度、费用和复盘；
- 能列出至少 10 个容易让回测产生幻觉的细节；
- 形成不超过 3 张策略表单，并给出明确证伪条件；
- 形成最小数据字段清单和仍未知的字段；
- 真实订单不是通过条件，资格不明时必须跳过。

通过后进入 `R1 本地小样本`：重建只读采集器，验证字段、频率、mapping 和数据质量。未通过则继续观察，
不申请云服务器、不建设 ClickHouse、不启用自动下单。

## 9. 负责人本轮需要决定

开始前只需要四个决定：

1. 首选 venue：默认 Polymarket 公共盘口；若地域/账户不允许，则全程 Paper；
2. 三类样本市场：短周期 crypto、规则清晰的普通二元市场、一组关联市场；
3. 是否允许第 4 局真实订单；默认 **不允许**；
4. 若允许，给出一次性总学费上限和单笔上限，执行中不可提高。

本指南不构成投资或法律建议。任何真实资金操作均需由账户持有人亲自确认规则、资格、订单和风险。

