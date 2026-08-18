import {
  asPositiveNumber,
  createNdjsonWriter,
  defaultOutputPath,
  installStopSignals,
  makeSessionId,
  parseArgs,
} from "./common.mjs";
import { collectPolymarket, discoverPolymarketMarkets } from "./polymarket.mjs";
import { createStopState } from "./ws-collector.mjs";
import { createDnsLookup, dnsModeFromArgs } from "./network.mjs";

const args = parseArgs(process.argv.slice(2));
const durationSeconds = asPositiveNumber(args.duration, 60, "duration");
const sessionId = String(args.session ?? makeSessionId("polymarket"));
const dnsMode = dnsModeFromArgs(args);
const lookup = createDnsLookup(dnsMode);
let assetIds = String(args["asset-ids"] ?? "").split(",").map((x) => x.trim()).filter(Boolean);
let selectedMarket = null;

if (assetIds.length === 0) {
  const query = String(args.query ?? args["market-query"] ?? "bitcoin");
  const markets = await discoverPolymarketMarkets({ query, limit: 500, lookup });
  selectedMarket = markets[0] ?? null;
  if (!selectedMarket) {
    throw new Error(`No active Polymarket order-book market matched query: ${query}`);
  }
  assetIds = selectedMarket.assetIds;
}

const assetLabels = new Map();
if (selectedMarket) {
  selectedMarket.assetIds.forEach((assetId, index) => {
    const outcome = selectedMarket.outcomes[index] ?? `outcome-${index}`;
    assetLabels.set(String(assetId), `${selectedMarket.slug}:${outcome}`);
  });
}

const writer = await createNdjsonWriter(args.output ?? defaultOutputPath(sessionId));
const stopState = createStopState();
const removeSignals = installStopSignals((reason) => stopState.stop(reason));

try {
  await collectPolymarket({
    sessionId,
    assetIds,
    assetLabels,
    durationMs: durationSeconds * 1_000,
    writer,
    stopState,
    lookup,
  });
} finally {
  removeSignals();
  await writer.close();
}

console.log(JSON.stringify({
  sessionId,
  source: "polymarket",
  market: selectedMarket,
  assetIds,
  dnsMode,
  rows: writer.rows,
  output: writer.path,
}, null, 2));
