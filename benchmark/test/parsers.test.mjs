import test from "node:test";
import assert from "node:assert/strict";
import { parseBinanceMessage } from "../src/binance.mjs";
import { parsePolymarketMessage } from "../src/polymarket.mjs";

test("Binance parser normalizes microsecond timestamps and sequences", () => {
  const parse = parseBinanceMessage({ sessionId: "s1", symbol: "BTCUSDT" });
  const [record] = parse({
    stream: "btcusdt@trade",
    data: {
      e: "trade",
      E: 1_787_000_000_123_000,
      T: 1_787_000_000_122_000,
      s: "BTCUSDT",
      t: 42,
      p: "64000.0",
      q: "0.1",
    },
  }, { wallMs: 1_787_000_000_130, monoNs: "100" });

  assert.equal(record.arrival_latency_ms, 7);
  assert.equal(record.sequence_start, 42);
  assert.equal(record.sequence_end, 42);
});

test("Polymarket parser handles array snapshots and string timestamps", () => {
  const parse = parsePolymarketMessage({
    sessionId: "s2",
    assetLabels: new Map([["abc", "market:Yes"]]),
  });
  const records = parse([{ event_type: "best_bid_ask", asset_id: "abc", timestamp: "1787000000000" }], {
    wallMs: 1_787_000_000_010,
    monoNs: "200",
  });

  assert.equal(records.length, 1);
  assert.equal(records[0].instrument, "market:Yes");
  assert.equal(records[0].arrival_latency_ms, 10);
});

test("Polymarket book timestamp is reported as snapshot age", () => {
  const parse = parsePolymarketMessage({
    sessionId: "s4",
    allowedAssetIds: new Set(["abc"]),
  });
  const [record] = parse({ event_type: "book", asset_id: "abc", timestamp: "1787000000000" }, {
    wallMs: 1_787_000_000_250,
    monoNs: "400",
  });

  assert.equal(record.arrival_latency_ms, null);
  assert.equal(record.snapshot_age_ms, 250);
  assert.equal(parse({ event_type: "new_market", timestamp: "1787000000000" }, {
    wallMs: 1_787_000_000_250,
    monoNs: "401",
  }).length, 0);
});

test("Binance BBO update id is not treated as a contiguous sequence", () => {
  const parse = parseBinanceMessage({ sessionId: "s3", symbol: "BTCUSDT" });
  const [record] = parse({
    stream: "btcusdt@bookTicker",
    data: { u: 123, s: "BTCUSDT", b: "1", B: "2", a: "3", A: "4" },
  }, { wallMs: 1_787_000_000_010, monoNs: "300" });

  assert.equal(record.sequence_start, null);
  assert.equal(record.sequence_end, null);
});
