#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const path = new URL("../config/market-universe.json", import.meta.url);
const config = JSON.parse(await readFile(path, "utf8"));
const problems = [];

if (config.schema_version !== 1) problems.push("unsupported config schema_version");
if (config.execution?.mode !== "disabled") problems.push("execution.mode must be disabled");
if (config.execution?.live_enabled !== false) problems.push("execution.live_enabled must be false");
if (config.sources?.predict_fun?.environment !== "testnet") {
  problems.push("Predict.fun must remain on testnet before G3");
}
if (config.sources?.predict_fun?.access_mode !== "read_only") {
  problems.push("Predict.fun access_mode must remain read_only before G3");
}
if (config.sources?.polymarket?.access_mode !== "public_read_only") {
  problems.push("Polymarket access_mode must remain public_read_only before G3");
}
if (config.retention?.wal_hours !== 72) problems.push("WAL retention must remain 72 hours");
if (config.retention?.clickhouse_hot_days !== 60) {
  problems.push("ClickHouse hot retention must remain 60 days");
}
if (config.retention?.object_storage_raw_months < 12) {
  problems.push("Raw object retention cannot be less than 12 months");
}
const segmentTarget = config.retention?.segment_target_mib;
if (segmentTarget?.default < 64 || segmentTarget?.maximum > 256) {
  problems.push("segment targets must stay within the approved 64–256 MiB range");
}

const serialized = JSON.stringify(config).toLowerCase();
for (const forbidden of ["private_key", "secret_key", "api_secret", "mnemonic"]) {
  if (serialized.includes(forbidden)) problems.push(`forbidden credential field: ${forbidden}`);
}

if (problems.length > 0) {
  console.error(`live-safety check failed:\n- ${problems.join("\n- ")}`);
  process.exitCode = 1;
} else {
  console.log("live-safety: PASS (all venue writes disabled)");
}
