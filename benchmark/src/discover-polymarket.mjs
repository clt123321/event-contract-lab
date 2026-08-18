import { asPositiveNumber, parseArgs } from "./common.mjs";
import { discoverPolymarketMarkets } from "./polymarket.mjs";
import { createDnsLookup, dnsModeFromArgs } from "./network.mjs";

const args = parseArgs(process.argv.slice(2));
const query = String(args.query ?? "bitcoin");
const outputLimit = asPositiveNumber(args.limit, 10, "limit");
const scanLimit = asPositiveNumber(args["scan-limit"], Math.max(200, outputLimit), "scan-limit");
const dnsMode = dnsModeFromArgs(args);
const lookup = createDnsLookup(dnsMode);
const markets = await discoverPolymarketMarkets({ query, limit: scanLimit, lookup });

console.log(JSON.stringify({ dnsMode, markets: markets.slice(0, outputLimit) }, null, 2));
