import { createReadStream } from "node:fs";
import { resolve } from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import { parseArgs, summarizeNumbers } from "./common.mjs";

function groupKey(record) {
  return [record.source ?? "unknown", record.stream ?? "unknown", record.instrument ?? "unknown", record.event_type ?? "unknown"].join("|");
}

function roundMetrics(metrics) {
  return Object.fromEntries(Object.entries(metrics).map(([key, value]) => [
    key,
    typeof value === "number" ? Math.round(value * 1_000) / 1_000 : value,
  ]));
}

export async function summarizeFile(inputPath, { clockOffsetMs = null } = {}) {
  const absolutePath = resolve(inputPath);
  const reader = createInterface({
    input: createReadStream(absolutePath, { encoding: "utf8" }),
    crlfDelay: Infinity,
  });

  const groups = new Map();
  const connections = new Map();
  let rows = 0;
  let invalidJson = 0;
  let firstRecv = null;
  let lastRecv = null;

  for await (const line of reader) {
    if (!line.trim()) continue;
    rows += 1;
    let record;
    try {
      record = JSON.parse(line);
    } catch {
      invalidJson += 1;
      continue;
    }

    if (Number.isFinite(record.recv_wall_ts_ms)) {
      firstRecv = firstRecv === null ? record.recv_wall_ts_ms : Math.min(firstRecv, record.recv_wall_ts_ms);
      lastRecv = lastRecv === null ? record.recv_wall_ts_ms : Math.max(lastRecv, record.recv_wall_ts_ms);
    }

    if (record.record_kind === "connection") {
      const key = `${record.source ?? "unknown"}|${record.stream ?? "unknown"}`;
      const entry = connections.get(key) ?? { opens: 0, closes: 0, errors: 0, parseErrors: 0 };
      if (record.event_type === "connection_open") entry.opens += 1;
      if (record.event_type === "connection_close") entry.closes += 1;
      if (record.event_type === "connection_error") entry.errors += 1;
      if (record.event_type === "parse_error") entry.parseErrors += 1;
      connections.set(key, entry);
      continue;
    }

    if (record.record_kind !== "market_data") continue;
    const key = groupKey(record);
    const entry = groups.get(key) ?? {
      count: 0,
      latencies: [],
      correctedLatencies: [],
      snapshotAges: [],
      missingSourceTimestamp: 0,
      negativeLatency: 0,
      overOneSecond: 0,
      sequenceGaps: 0,
      sequenceOverlaps: 0,
      lastSequenceEnd: null,
    };
    entry.count += 1;

    if (Number.isFinite(record.arrival_latency_ms)) {
      entry.latencies.push(record.arrival_latency_ms);
      if (Number.isFinite(clockOffsetMs)) {
        entry.correctedLatencies.push(record.arrival_latency_ms + clockOffsetMs);
      }
      if (record.arrival_latency_ms < 0) entry.negativeLatency += 1;
      if (record.arrival_latency_ms > 1_000) entry.overOneSecond += 1;
    } else if (Number.isFinite(record.snapshot_age_ms)) {
      entry.snapshotAges.push(record.snapshot_age_ms);
    } else {
      entry.missingSourceTimestamp += 1;
    }

    const hasSequence = record.sequence_start !== null && record.sequence_start !== undefined
      && record.sequence_end !== null && record.sequence_end !== undefined;
    const start = hasSequence ? Number(record.sequence_start) : null;
    const end = hasSequence ? Number(record.sequence_end) : null;
    if (hasSequence && Number.isSafeInteger(start) && Number.isSafeInteger(end)) {
      if (entry.lastSequenceEnd !== null) {
        if (start > entry.lastSequenceEnd + 1) entry.sequenceGaps += start - entry.lastSequenceEnd - 1;
        if (end <= entry.lastSequenceEnd) entry.sequenceOverlaps += 1;
      }
      entry.lastSequenceEnd = Math.max(entry.lastSequenceEnd ?? end, end);
    }
    groups.set(key, entry);
  }

  return {
    input: absolutePath,
    rows,
    invalidJson,
    observedDurationSeconds: firstRecv === null || lastRecv === null ? null : (lastRecv - firstRecv) / 1_000,
    connections: Object.fromEntries([...connections.entries()].sort()),
    clockOffsetMs,
    groups: Object.fromEntries([...groups.entries()].sort().map(([key, value]) => {
      const summary = {
        count: value.count,
        arrivalLatencyMs: roundMetrics(summarizeNumbers(value.latencies)),
        snapshotAgeMs: roundMetrics(summarizeNumbers(value.snapshotAges)),
        missingSourceTimestamp: value.missingSourceTimestamp,
        negativeLatency: value.negativeLatency,
        overOneSecond: value.overOneSecond,
        sequenceGaps: value.sequenceGaps,
        sequenceOverlaps: value.sequenceOverlaps,
      };
      if (Number.isFinite(clockOffsetMs)) {
        summary.clockCorrectedArrivalLatencyMs = roundMetrics(summarizeNumbers(value.correctedLatencies));
      }
      return [key, summary];
    })),
  };
}

if (fileURLToPath(import.meta.url) === resolve(process.argv[1])) {
  const args = parseArgs(process.argv.slice(2));
  if (!args.input) throw new Error("--input is required");
  const clockOffsetMs = args["clock-offset-ms"] === undefined ? null : Number(args["clock-offset-ms"]);
  if (clockOffsetMs !== null && !Number.isFinite(clockOffsetMs)) {
    throw new Error("--clock-offset-ms must be a finite number");
  }
  console.log(JSON.stringify(await summarizeFile(args.input, { clockOffsetMs }), null, 2));
}
