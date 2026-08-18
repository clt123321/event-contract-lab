import {
  asPositiveNumber,
  createNdjsonWriter,
  defaultOutputPath,
  installStopSignals,
  makeSessionId,
  parseArgs,
} from "./common.mjs";
import { collectBinance } from "./binance.mjs";
import { collectPolymarket, discoverPolymarketMarkets } from "./polymarket.mjs";
import { createStopState } from "./ws-collector.mjs";
import { createDnsLookup, dnsModeFromArgs } from "./network.mjs";

const args = parseArgs(process.argv.slice(2));
const durationSeconds = asPositiveNumber(args.duration, 60, "duration");
const symbol = String(args.symbol ?? "BTCUSDT").toUpperCase();
const binanceBaseUrl = String(args["binance-base-url"] ?? process.env.BINANCE_WS_BASE ?? "wss://data-stream.binance.vision");
const polymarketQuery = String(args["polymarket-query"] ?? "bitcoin");
const sessionId = String(args.session ?? makeSessionId("public"));
const dnsMode = dnsModeFromArgs(args);
const lookup = createDnsLookup(dnsMode);

const markets = await discoverPolymarketMarkets({ query: polymarketQuery, limit: 500, lookup });
const selectedMarket = markets[0] ?? null;
if (!selectedMarket) {
  throw new Error(`No active Polymarket order-book market matched query: ${polymarketQuery}`);
}

const assetLabels = new Map();
selectedMarket.assetIds.forEach((assetId, index) => {
  const outcome = selectedMarket.outcomes[index] ?? `outcome-${index}`;
  assetLabels.set(String(assetId), `${selectedMarket.slug}:${outcome}`);
});

const writer = await createNdjsonWriter(args.output ?? defaultOutputPath(sessionId));
const stopState = createStopState();
const removeSignals = installStopSignals((reason) => stopState.stop(reason));

try {
  await Promise.all([
    collectBinance({
      sessionId,
      symbol,
      baseUrl: binanceBaseUrl,
      durationMs: durationSeconds * 1_000,
      writer,
      stopState,
      lookup,
    }),
    collectPolymarket({
      sessionId,
      assetIds: selectedMarket.assetIds,
      assetLabels,
      durationMs: durationSeconds * 1_000,
      writer,
      stopState,
      lookup,
    }),
  ]);
} finally {
  removeSignals();
  stopState.stop("complete");
  await writer.close();
}

console.log(JSON.stringify({
  sessionId,
  durationSeconds,
  binanceSymbol: symbol,
  binanceBaseUrl,
  dnsMode,
  polymarket: selectedMarket,
  rows: writer.rows,
  output: writer.path,
}, null, 2));
