import { mkdir } from "node:fs/promises";
import { createWriteStream } from "node:fs";
import { dirname, resolve } from "node:path";

export function parseArgs(argv) {
  const out = { _: [] };
  for (let i = 0; i < argv.length; i += 1) {
    const item = argv[i];
    if (!item.startsWith("--")) {
      out._.push(item);
      continue;
    }
    const eq = item.indexOf("=");
    if (eq !== -1) {
      out[item.slice(2, eq)] = item.slice(eq + 1);
      continue;
    }
    const key = item.slice(2);
    const next = argv[i + 1];
    if (next !== undefined && !next.startsWith("--")) {
      out[key] = next;
      i += 1;
    } else {
      out[key] = true;
    }
  }
  return out;
}

export function asPositiveNumber(value, fallback, name) {
  if (value === undefined) return fallback;
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`${name} must be a positive number`);
  }
  return parsed;
}

export function normalizeEpochMs(value) {
  if (value === null || value === undefined || value === "") return null;
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) return null;
  if (numeric >= 1e17) return numeric / 1e6; // nanoseconds
  if (numeric >= 1e14) return numeric / 1e3; // microseconds
  if (numeric >= 1e11) return numeric; // milliseconds
  if (numeric >= 1e9) return numeric * 1e3; // seconds
  return null;
}

export function makeSessionId(prefix = "capture") {
  const stamp = new Date().toISOString().replace(/[-:.]/g, "").replace("Z", "Z");
  const entropy = Math.random().toString(16).slice(2, 8);
  return `${prefix}-${stamp}-${entropy}`;
}

export function defaultOutputPath(sessionId) {
  return resolve("data", "raw", `${sessionId}.ndjson`);
}

export async function createNdjsonWriter(outputPath) {
  const absolutePath = resolve(outputPath);
  await mkdir(dirname(absolutePath), { recursive: true });
  const stream = createWriteStream(absolutePath, { flags: "a", encoding: "utf8" });
  let rows = 0;
  let closed = false;

  return {
    path: absolutePath,
    get rows() {
      return rows;
    },
    write(record) {
      if (closed) throw new Error("NDJSON writer is already closed");
      rows += 1;
      stream.write(`${JSON.stringify(record)}\n`);
    },
    async close() {
      if (closed) return;
      closed = true;
      await new Promise((resolveClose, rejectClose) => {
        stream.once("error", rejectClose);
        stream.end(resolveClose);
      });
    },
  };
}

export function receiveClock() {
  return {
    wallMs: Date.now(),
    monoNs: process.hrtime.bigint().toString(),
  };
}

export function percentile(sortedValues, probability) {
  if (sortedValues.length === 0) return null;
  if (sortedValues.length === 1) return sortedValues[0];
  const index = (sortedValues.length - 1) * probability;
  const lower = Math.floor(index);
  const upper = Math.ceil(index);
  if (lower === upper) return sortedValues[lower];
  const weight = index - lower;
  return sortedValues[lower] * (1 - weight) + sortedValues[upper] * weight;
}

export function summarizeNumbers(values) {
  const finite = values.filter(Number.isFinite).sort((a, b) => a - b);
  if (finite.length === 0) {
    return { count: 0, min: null, p50: null, p95: null, p99: null, max: null, mean: null };
  }
  const sum = finite.reduce((acc, value) => acc + value, 0);
  return {
    count: finite.length,
    min: finite[0],
    p50: percentile(finite, 0.5),
    p95: percentile(finite, 0.95),
    p99: percentile(finite, 0.99),
    max: finite.at(-1),
    mean: sum / finite.length,
  };
}

export async function messageDataToText(data) {
  if (typeof data === "string") return data;
  if (data instanceof ArrayBuffer) return Buffer.from(data).toString("utf8");
  if (ArrayBuffer.isView(data)) {
    return Buffer.from(data.buffer, data.byteOffset, data.byteLength).toString("utf8");
  }
  if (data && typeof data.text === "function") return data.text();
  return String(data);
}

export function sleep(ms) {
  return new Promise((resolveSleep) => setTimeout(resolveSleep, ms));
}

export function installStopSignals(stop) {
  const handler = () => stop("signal");
  process.once("SIGINT", handler);
  process.once("SIGTERM", handler);
  return () => {
    process.removeListener("SIGINT", handler);
    process.removeListener("SIGTERM", handler);
  };
}
