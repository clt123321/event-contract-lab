import test from "node:test";
import assert from "node:assert/strict";
import { normalizeEpochMs, parseArgs, percentile, summarizeNumbers } from "../src/common.mjs";

test("parseArgs handles spaced and equals forms", () => {
  assert.deepEqual(parseArgs(["--duration", "10", "--symbol=BTCUSDT", "file"]), {
    _: ["file"],
    duration: "10",
    symbol: "BTCUSDT",
  });
});

test("normalizeEpochMs handles seconds, milliseconds, microseconds and nanoseconds", () => {
  assert.equal(normalizeEpochMs(1_787_000_000), 1_787_000_000_000);
  assert.equal(normalizeEpochMs(1_787_000_000_123), 1_787_000_000_123);
  assert.equal(normalizeEpochMs(1_787_000_000_123_000), 1_787_000_000_123);
  assert.equal(normalizeEpochMs(1_787_000_000_123_000_000), 1_787_000_000_123);
  assert.equal(normalizeEpochMs(null), null);
});

test("percentile interpolates and summarizeNumbers reports distribution", () => {
  assert.equal(percentile([1, 2, 3, 4], 0.5), 2.5);
  assert.deepEqual(summarizeNumbers([1, 2, 3, 4]), {
    count: 4,
    min: 1,
    p50: 2.5,
    p95: 3.8499999999999996,
    p99: 3.9699999999999998,
    max: 4,
    mean: 2.5,
  });
});
