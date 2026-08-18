import {
  asPositiveNumber,
  createNdjsonWriter,
  defaultOutputPath,
  makeSessionId,
  parseArgs,
  receiveClock,
} from "./common.mjs";
import {
  discoverPolymarketMarkets,
  fetchPolymarketPublicSnapshot,
  polymarketSnapshotRecords,
} from "./polymarket.mjs";
import { createDnsLookup, dnsModeFromArgs } from "./network.mjs";

const args = parseArgs(process.argv.slice(2));
const query = String(args.query ?? "bitcoin");
const scanLimit = asPositiveNumber(args["scan-limit"], 500, "scan-limit");
const tradeLimit = asPositiveNumber(args["trade-limit"], 100, "trade-limit");
const sessionId = String(args.session ?? makeSessionId("polymarket-snapshot"));
const dnsMode = dnsModeFromArgs(args);
const lookup = createDnsLookup(dnsMode);

const markets = await discoverPolymarketMarkets({ query, limit: scanLimit, lookup });
const requestedMarketId = args["market-id"] === undefined ? null : String(args["market-id"]);
const market = requestedMarketId === null
  ? markets[0]
  : markets.find((candidate) => String(candidate.id) === requestedMarketId);

if (!market) {
  throw new Error(requestedMarketId === null
    ? `No active Polymarket order-book market matched query: ${query}`
    : `Polymarket market ${requestedMarketId} was not found in query results for: ${query}`);
}

const snapshot = await fetchPolymarketPublicSnapshot({ market, lookup, tradeLimit });
const records = polymarketSnapshotRecords({ sessionId, snapshot, clock: receiveClock() });
const writer = await createNdjsonWriter(args.output ?? defaultOutputPath(sessionId));
try {
  for (const record of records) writer.write(record);
} finally {
  await writer.close();
}

console.log(JSON.stringify({
  sessionId,
  source: "polymarket",
  market: { id: market.id, conditionId: market.conditionId, slug: market.slug },
  assetIds: market.assetIds,
  dnsMode,
  orderbooks: snapshot.orderbooks.length,
  trades: snapshot.trades.length,
  rows: writer.rows,
  output: writer.path,
}, null, 2));
