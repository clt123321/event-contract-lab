import { asPositiveNumber, parseArgs, summarizeNumbers } from "./common.mjs";
import { createDnsLookup, dnsModeFromArgs, requestJson } from "./network.mjs";

const args = parseArgs(process.argv.slice(2));
const sampleCount = Math.floor(asPositiveNumber(args.samples, 10, "samples"));
const endpoint = String(args.endpoint ?? "https://api.binance.com/api/v3/time");
const dnsMode = dnsModeFromArgs(args);
const lookup = createDnsLookup(dnsMode);
const samples = [];

for (let index = 0; index < sampleCount; index += 1) {
  const wallStartMs = Date.now();
  const monoStartNs = process.hrtime.bigint();
  const payload = await requestJson(endpoint, { lookup, timeoutMs: 10_000 });
  const monoEndNs = process.hrtime.bigint();
  const serverTimeMs = Number(payload.serverTime);
  if (!Number.isFinite(serverTimeMs)) throw new Error("Clock probe response has no numeric serverTime");

  const roundTripMs = Number(monoEndNs - monoStartNs) / 1e6;
  const localMidpointMs = wallStartMs + roundTripMs / 2;
  samples.push({
    index,
    roundTripMs,
    serverMinusLocalMs: serverTimeMs - localMidpointMs,
  });
}

const best = [...samples].sort((a, b) => a.roundTripMs - b.roundTripMs)[0];
console.log(JSON.stringify({
  endpoint,
  dnsMode,
  samples: sampleCount,
  roundTripMs: summarizeNumbers(samples.map((sample) => sample.roundTripMs)),
  serverMinusLocalMs: summarizeNumbers(samples.map((sample) => sample.serverMinusLocalMs)),
  recommendedClockOffsetMs: best.serverMinusLocalMs,
  recommendation: "Pass recommendedClockOffsetMs to summarize as --clock-offset-ms.",
  raw: samples,
}, null, 2));
