# Canonical Silver 数据模型 v1

版本：v0.2｜更新时间：2026-08-20｜状态：Implemented locally

## 1. 边界

`RawEventEnvelope` 是不可变证据，`CanonicalMarketEvent` 是可重建的统一事实。归一化器只映射
payload 中有明确语义的字段，不根据名称猜测 market/outcome，也不做跨 venue 映射。

```text
sealed Raw NDJSON
  → validate Raw contract
  → source/event parser
  → timestamp/sequence/book/price quality rules
  ├─ accepted or warned → canonical.ndjson
  ├─ rejected           → quarantine.ndjson
  └─ valid non-market   → skipped count; remains replayable in Raw
  → quality.json + transform-manifest.json
```

只有最后的 transform manifest 存在且其 checksum 全部匹配，转换才视为完成。中断后产生但
没有 manifest 的文件不是正式 Silver 数据。

## 2. Canonical v1 字段原则

- `available_at_ms` 使用本系统接收时间，是 point-in-time 回放的可见时间；不能用 source time
  替代，避免未来数据泄漏。
- `source_event_ts_ms` 只表示源提供的时间，允许为空。
- price/quantity 和盘口档位使用十进制字符串，保留源精度；落 ClickHouse/Parquet 时才按已
  审批 precision/scale 转换。
- `market_id`、`outcome_id` 只从 Raw 顶层显式字段继承；缺失时保持 null。
- `canonical_event_id = sha256(raw line) + emission index`。当前每条 Raw 最多产生一条
  canonical event，index 固定为 0，为未来一对多解析保留空间。
- lineage 同时保存输入文件 SHA-256、物理行号、Raw 行 SHA-256 和 Raw schema version。

支持的首批映射：

| Source | Raw event type | Canonical kind |
|---|---|---|
| Binance | `trade` | `trade` |
| Binance | `bookTicker` | `best_bid_ask` |
| Binance | `depthUpdate` | `book_delta` |
| Polymarket | `trade` / `last_trade_price` | `trade` |
| Polymarket | `best_bid_ask` | `best_bid_ask` |
| Polymarket | `book` | `book_snapshot` |
| Polymarket | `price_change` | `book_delta` |

未支持的 source/event 进入 quarantine，不静默降级成 `unknown` Silver 行。
`connection`、`market_metadata` 等合法但不属于 Canonical Market v1 的 record kind 计入
`skipped_rows/skip_counts`，不算坏数据；后续由各自事实转换器从同一 Raw 重建。

## 3. Quality v1

策略文件是 `config/quality-policy.v1.json`。改变既有字段含义或处置规则时新增版本，不覆写
历史正式数据所引用的策略。

直接隔离：无效 Raw 契约、无效 JSON、缺少必要字段、非数值/非正交易值、重复 Raw、session
内 sequence 回退、BBO/完整快照交叉、Polymarket 概率价格越界、超过容忍度的未来时间戳。
单条 book delta 不足以证明盘口交叉，必须由后续有状态 book builder 判断。

保留并标记：缺失源时间、轻微负延迟、陈旧事件、空/单边盘口、sequence gap。正式研究数据集
是否排除某个 warning，由之后版本化的 quality mask 决定，notebook 不得手工删除。

当前需人工确认的语义只有两类：

1. 哪些 warning 可以进入特定研究数据集；
2. Polymarket/Predict.fun 的 market/outcome 映射证据与审批。

## 4. 本地运行

```bash
make normalize \
  INPUT=data/wal/<sealed-segment>.ndjson \
  OUTPUT_DIR=data/silver/<unique-run-id>
```

目标目录必须是新的；工具拒绝覆盖已有文件。`transform-manifest.json` 绑定输入、canonical、
quarantine、quality policy、quality report、normalizer version 和 Git commit。

`schemas/clickhouse/canonical_market_event_v1.sql` 是候选 DDL，不表示 ClickHouse 已部署。
Parquet v1 和 Dataset Manifest v2 已复用同一 canonical schema、quality policy 和
transform identity，详见 [`DATASET_AND_REPLAY.md`](DATASET_AND_REPLAY.md)。
