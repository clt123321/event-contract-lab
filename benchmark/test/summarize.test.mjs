import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { summarizeFile } from "../src/summarize.mjs";

test("summary reports raw and clock-corrected latency distributions", async () => {
  const directory = await mkdtemp(join(tmpdir(), "latency-summary-"));
  const input = join(directory, "sample.ndjson");
  const base = {
    schema_version: 1,
    record_kind: "market_data",
    source: "binance",
    stream: "btcusdt@trade",
    instrument: "BTCUSDT",
    event_type: "trade",
    source_event_ts_ms: 990,
    sequence_start: 1,
    sequence_end: 1,
  };
  await writeFile(input, [
    JSON.stringify({ ...base, recv_wall_ts_ms: 1_000, arrival_latency_ms: 10 }),
    JSON.stringify({ ...base, recv_wall_ts_ms: 1_020, arrival_latency_ms: 20, sequence_start: 2, sequence_end: 2 }),
  ].join("\n"));

  try {
    const summary = await summarizeFile(input, { clockOffsetMs: -3 });
    const group = summary.groups["binance|btcusdt@trade|BTCUSDT|trade"];
    assert.equal(group.arrivalLatencyMs.p50, 15);
    assert.equal(group.clockCorrectedArrivalLatencyMs.p50, 12);
    assert.equal(group.sequenceGaps, 0);
    assert.equal(group.sequenceOverlaps, 0);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
