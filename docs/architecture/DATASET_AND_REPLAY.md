# Parquet、Dataset Manifest 与 Replay v1

版本：v0.1｜更新时间：2026-08-20｜状态：Implemented locally

## 1. 可复现链路

```text
transform-manifest.json
  → verify canonical NDJSON SHA-256/bytes/rows
  → apply versioned quality mask
  → write fixed-schema ZSTD Parquet
  → read Parquet back and compare every CanonicalMarketEvent
  → dataset-manifest.v2.json
  → verify Parquet SHA-256/bytes/rows
  → stable point-in-time ordering + virtual clock
  → replay.ndjson + replay-manifest.json
```

Parquet 使用 Apache Arrow Rust `59.2.0`，固定 `created_by`、ZSTD 和 65,536 行 row group，
测试要求相同输入产生字节一致的 Parquet 和 Dataset Manifest。

## 2. Parquet v1

列式字段包括 source、stream、session、instrument、market/outcome、source/available time、
sequence、price/quantity/BBO、lineage。盘口数组和 quality flags 使用确定性 JSON 字符串列；
同时保存 `canonical_json` 作为无损回读列。写入后必须从 Parquet 反序列化并与输入事件逐条相等。

`canonical_json` 是 v1 的兼容性护栏，不是长期查询接口。后续经真实容量 benchmark 批准
decimal scale 和 nested-list 兼容性后，可新增 Parquet schema v2，不能重解释 v1。

## 3. Dataset Manifest v2

v1 草案包含创建时间和 replay seed，二者不应属于数据身份，因此实际实现使用 v2。Dataset ID
绑定 transform manifest SHA/ID、Parquet SHA、quality mask SHA、代码提交和 builder version。

严格质量掩码 `research-strict-v1` 默认不允许任何 warning。被排除行不会删除，仍保留在 Silver
和 Raw；manifest 记录 `input/included/excluded` 行数及逐 flag 排除计数。

当前 builder 一次冻结一个 transform manifest 和一个 Parquet 文件。这足以建立正确契约；
多 segment 分区/合并将在得到本地长采集数据后扩展，Dataset Manifest v2 已使用数组为此预留。

## 4. Replay v1

Replay 的首要时间是 `available_at_ms`，即事件当时对系统可见的时间，禁止用可能提前暴露未来
信息的 source timestamp 排序。相同可见时间的稳定顺序为：

```text
source → session_id → numeric recv_mono_ns → canonical_event_id
```

输出 frame 包含连续 `replay_sequence`、`virtual_time_ms`、相对首事件的
`elapsed_virtual_ms` 和完整 canonical event。Replay Manifest 绑定 dataset/config/seed/code/output。
v1 尚不包含 book state、费用、滑点、订单或 fill；这些属于下一阶段 replay consumer/paper OMS。

## 5. 命令

```bash
make dataset \
  TRANSFORM_MANIFEST=data/silver/<run>/transform-manifest.json \
  OUTPUT_DIR=data/datasets/<new-run>

make replay \
  DATASET_MANIFEST=data/datasets/<run>/dataset-manifest.json \
  OUTPUT_DIR=data/replays/<new-run>
```

所有输出路径必须是新的，工具拒绝覆盖。`make verify-local` 使用合成质量夹具完整覆盖
Raw/WAL/Silver/Parquet/Dataset/Replay；正式数据仍需人工批准市场范围和 quality mask。
