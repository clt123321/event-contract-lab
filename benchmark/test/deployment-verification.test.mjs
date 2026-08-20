import assert from "node:assert/strict";
import test from "node:test";

import {
  compareReports,
  evaluateCaptureSummary,
  evaluateClock,
  evaluateDatasetReplay,
  evaluateHostNormalization,
  evaluateHostDatasetReplay,
  evaluateNetwork,
  evaluateNormalization,
  evaluateWalImport,
  overallStatus,
} from "../../scripts/lib/verification.mjs";

test("WAL evaluation binds imported and verified row counts", () => {
  const checks = evaluateWalImport(
    { status: "sealed", imported_rows: 2 },
    { status: "verified", rows: 2, segments: 1 },
    2,
  );
  assert.equal(overallStatus(checks), "passed");
  assert.equal(overallStatus(evaluateWalImport(
    { status: "sealed", imported_rows: 2 },
    { status: "verified", rows: 1, segments: 1 },
    2,
  )), "failed");
});

test("normalization evaluation binds rows, input, policy and code identity", () => {
  const checks = evaluateNormalization(
    { status: "normalized" },
    {
      input_sha256: "input",
      quality_policy_sha256: "policy",
      summary: {
        input_rows: 6, canonical_rows: 2, quarantined_rows: 4, skipped_rows: 0, warning_rows: 1,
      },
    },
    {
      transform_id: "a".repeat(64),
      input: { sha256: "input" },
      quality_policy_sha256: "policy",
      code_commit: "commit",
    },
    { inputRows: 6, canonicalRows: 2, quarantinedRows: 4, skippedRows: 0, warningRows: 1 },
    "commit",
  );
  assert.equal(overallStatus(checks), "passed");
});

test("host normalization enforces canonical volume and quarantine ratio", () => {
  const quality = {
    input_sha256: "input",
    quality_policy_sha256: "policy",
    summary: { input_rows: 1_000, canonical_rows: 990, quarantined_rows: 5, skipped_rows: 5 },
  };
  const manifest = {
    input: { sha256: "input" },
    quality_policy_sha256: "policy",
    code_commit: "commit",
  };
  const profile = { minimum_canonical_rows: 10, maximum_quarantine_ratio: 0.01 };
  assert.equal(overallStatus(evaluateHostNormalization(
    { status: "normalized" }, quality, manifest, profile, "commit",
  )), "passed");
  quality.summary.quarantined_rows = 20;
  quality.summary.canonical_rows = 975;
  assert.equal(overallStatus(evaluateHostNormalization(
    { status: "normalized" }, quality, manifest, profile, "commit",
  )), "failed");
});

test("dataset and replay checks bind rows, hashes, IDs and code", () => {
  const profile = {
    dataset_input_rows: 2,
    dataset_included_rows: 1,
    dataset_excluded_rows: 1,
    replay_event_rows: 1,
  };
  const dataset = {
    dataset_id: "dataset",
    input_rows: 2,
    included_rows: 1,
    excluded_rows: 1,
    code_commit: "commit",
    parquet_files: [{ sha256: "a".repeat(64) }],
  };
  const replay = {
    dataset_id: "dataset",
    event_count: 1,
    output: { row_count: 1 },
    code_commit: "commit",
  };
  assert.equal(overallStatus(evaluateDatasetReplay(
    { status: "frozen" }, dataset, { status: "replayed" }, replay, profile, "commit",
  )), "passed");
});

test("host dataset/replay requires nonempty point-in-time output", () => {
  const dataset = {
    dataset_id: "dataset", input_rows: 10, included_rows: 8, excluded_rows: 2, code_commit: "commit",
  };
  const replay = { dataset_id: "dataset", event_count: 8, output: { row_count: 8 } };
  assert.equal(overallStatus(evaluateHostDatasetReplay(
    { status: "frozen" }, dataset, { status: "replayed" }, replay, "commit",
  )), "passed");
});

test("capture evaluation requires both sources and no silent parser errors", () => {
  const profile = {
    minimum_capture_rows: 10,
    maximum_invalid_json: 0,
    maximum_connection_errors: 0,
    maximum_parse_errors: 0,
    required_market_data_sources: ["binance", "polymarket"],
  };
  const summary = {
    rows: 12,
    invalidJson: 0,
    connections: { "binance|btc": { errors: 0, parseErrors: 0 } },
    groups: { "binance|trade|BTCUSDT|trade": {}, "polymarket|market|token|price_change": {} },
  };
  assert.equal(overallStatus(evaluateCaptureSummary(summary, profile)), "passed");
  summary.connections["polymarket|market"] = { errors: 0, parseErrors: 1 };
  assert.equal(overallStatus(evaluateCaptureSummary(summary, profile)), "failed");
});

test("host checks enforce clock and per-target protocol readiness", () => {
  const clockChecks = evaluateClock(
    { samples: 5, recommendedClockOffsetMs: -8 },
    { clock_samples: 5, maximum_absolute_clock_offset_ms: 100 },
  );
  assert.equal(overallStatus(clockChecks), "passed");

  const networkChecks = evaluateNetwork({ targets: [{
    id: "binance-market-data",
    dnsOverHttps: { ok: true },
    websocket: { ok: true },
  }] }, { required_network_targets: ["binance-market-data"] });
  assert.equal(overallStatus(networkChecks), "passed");
});

test("report comparison highlights fixes, regressions and profile drift", () => {
  const before = {
    run_id: "before",
    status: "failed",
    git: { commit: "a" },
    verification_profile: { sha256: "profile-a" },
    configuration: { market_universe: { sha256: "market-a" } },
    checks: [
      { id: "clock", passed: false },
      { id: "wal", passed: true },
    ],
    steps: [{ id: "network", duration_ms: 70_000 }],
  };
  const after = {
    run_id: "after",
    status: "failed",
    git: { commit: "b" },
    verification_profile: { sha256: "profile-a" },
    configuration: { market_universe: { sha256: "market-b" } },
    checks: [
      { id: "clock", passed: true },
      { id: "wal", passed: false },
    ],
    steps: [{ id: "network", duration_ms: 10_000 }],
  };
  const comparison = compareReports(before, after);
  assert.deepEqual(comparison.improved.map((item) => item.id), ["clock"]);
  assert.deepEqual(comparison.regressed.map((item) => item.id), ["wal"]);
  assert.equal(comparison.same_verification_profile, true);
  assert.equal(comparison.same_market_universe, false);
  assert.equal(comparison.duration_changes[0].delta_ms, -60_000);
});
