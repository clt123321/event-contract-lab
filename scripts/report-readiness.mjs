#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const path = new URL("../config/market-universe.json", import.meta.url);
const config = JSON.parse(await readFile(path, "utf8"));
const sources = config.sources ?? {};
const blockers = [];

const binanceSymbols = sources.binance?.symbols ?? [];
if (!binanceSymbols.includes("BTCUSDT") || !binanceSymbols.includes("ETHUSDT")) {
  blockers.push({ owner: "repository", gate: "P0", item: "Freeze Binance BTCUSDT and ETHUSDT" });
}

const predictMarkets = sources.predict_fun?.approved_market_ids ?? [];
if (predictMarkets.length === 0) {
  blockers.push({
    owner: "project_owner/external",
    gate: "P0",
    item: "Obtain Predict.fun Testnet/read-only permission and freeze BTC/ETH short-cycle market IDs",
  });
}

const polymarketMarkets = sources.polymarket?.approved_markets ?? [];
const minimumPolymarket = sources.polymarket?.approved_market_count?.minimum ?? 3;
const maximumPolymarket = sources.polymarket?.approved_market_count?.maximum ?? 5;
if (polymarketMarkets.length < minimumPolymarket || polymarketMarkets.length > maximumPolymarket) {
  blockers.push({
    owner: "project_owner",
    gate: "P0",
    item: `Review and freeze ${minimumPolymarket}–${maximumPolymarket} Polymarket markets`,
  });
}

blockers.push(
  { owner: "project_owner", gate: "G2-after-local-data-replay", item: "After local Parquet/replay/deploy-artifact gates pass, provide AWS project account, Tokyo deploy role, billing owner, and $100/$150 alert recipients" },
  { owner: "project_owner", gate: "G3", item: "Keep live execution blocked; legal/account/geography/risk approval is not granted" },
);

console.log(JSON.stringify({
  config_schema_version: config.schema_version,
  public_local_development_ready: binanceSymbols.length >= 2,
  formal_three_source_benchmark_ready: !blockers.some((item) => item.gate === "P0"),
  cloud_application_recommended_now: false,
  next_engineering_gate: "local 24h soak, deploy artifact/IaC review, then empty-ClickHouse recovery and paper replay consumer",
  cloud_deployment_ready: false,
  live_execution_enabled: config.execution?.live_enabled === true,
  blockers,
}, null, 2));
