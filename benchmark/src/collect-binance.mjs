import {
  asPositiveNumber,
  createNdjsonWriter,
  defaultOutputPath,
  installStopSignals,
  makeSessionId,
  parseArgs,
} from "./common.mjs";
import { collectBinance } from "./binance.mjs";
import { createStopState } from "./ws-collector.mjs";
import { createDnsLookup, dnsModeFromArgs } from "./network.mjs";

const args = parseArgs(process.argv.slice(2));
const durationSeconds = asPositiveNumber(args.duration, 60, "duration");
const symbol = String(args.symbol ?? "BTCUSDT").toUpperCase();
const baseUrl = String(args["base-url"] ?? process.env.BINANCE_WS_BASE ?? "wss://data-stream.binance.vision");
const dnsMode = dnsModeFromArgs(args);
const lookup = createDnsLookup(dnsMode);
const sessionId = String(args.session ?? makeSessionId("binance"));
const writer = await createNdjsonWriter(args.output ?? defaultOutputPath(sessionId));
const stopState = createStopState();
const removeSignals = installStopSignals((reason) => stopState.stop(reason));

try {
  await collectBinance({
    sessionId,
    symbol,
    baseUrl,
    lookup,
    durationMs: durationSeconds * 1_000,
    writer,
    stopState,
  });
} finally {
  removeSignals();
  await writer.close();
}

console.log(JSON.stringify({ sessionId, source: "binance", symbol, baseUrl, dnsMode, rows: writer.rows, output: writer.path }, null, 2));
