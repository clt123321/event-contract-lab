import { lookup } from "node:dns/promises";
import { mkdir, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import tls from "node:tls";
import { parseArgs } from "./common.mjs";

const args = parseArgs(process.argv.slice(2));
const timeoutMs = Number(args.timeout ?? 8_000);
if (!Number.isFinite(timeoutMs) || timeoutMs <= 0) throw new Error("--timeout must be a positive number");

const targets = [
  {
    id: "control-example",
    host: "example.com",
    port: 443,
    http: "https://example.com/",
  },
  {
    id: "clock-apple-ntp",
    host: "time.apple.com",
    port: 123,
  },
  {
    id: "binance-rest",
    host: "api.binance.com",
    port: 443,
    http: "https://api.binance.com/api/v3/time",
  },
  {
    id: "binance-market-data",
    host: "data-stream.binance.vision",
    port: 443,
    websocket: "wss://data-stream.binance.vision/ws/btcusdt@trade?timeUnit=MICROSECOND",
  },
  {
    id: "binance-stream-9443",
    host: "stream.binance.com",
    port: 9443,
    websocket: "wss://stream.binance.com:9443/ws/btcusdt@trade?timeUnit=MICROSECOND",
  },
  {
    id: "polymarket-gamma",
    host: "gamma-api.polymarket.com",
    port: 443,
    http: "https://gamma-api.polymarket.com/markets?active=true&closed=false&limit=1",
  },
  {
    id: "polymarket-market-ws",
    host: "ws-subscriptions-clob.polymarket.com",
    port: 443,
    websocket: "wss://ws-subscriptions-clob.polymarket.com/ws/market",
  },
];

function errorInfo(error) {
  return {
    name: error?.name ?? null,
    code: error?.code ?? null,
    message: String(error?.message ?? error),
  };
}

function elapsedMs(startNs) {
  return Math.round(Number(process.hrtime.bigint() - startNs) / 1e6 * 1_000) / 1_000;
}

function proxyInfo(name) {
  const value = process.env[name];
  if (!value) return null;
  try {
    const url = new URL(value);
    return {
      configured: true,
      protocol: url.protocol,
      host: url.hostname,
      port: url.port || null,
      hasCredentials: Boolean(url.username || url.password),
    };
  } catch {
    return { configured: true, parseableUrl: false };
  }
}

async function probeDns(host) {
  const startNs = process.hrtime.bigint();
  try {
    const addresses = await lookup(host, { all: true });
    return { ok: true, elapsedMs: elapsedMs(startNs), addresses };
  } catch (error) {
    return { ok: false, elapsedMs: elapsedMs(startNs), error: errorInfo(error) };
  }
}

async function probeDnsOverHttps(host) {
  const startNs = process.hrtime.bigint();
  const url = new URL("https://cloudflare-dns.com/dns-query");
  url.searchParams.set("name", host);
  url.searchParams.set("type", "A");
  try {
    const response = await fetch(url, {
      headers: { accept: "application/dns-json" },
      signal: AbortSignal.timeout(timeoutMs),
    });
    const payload = await response.json();
    return {
      ok: response.ok && payload.Status === 0,
      elapsedMs: elapsedMs(startNs),
      status: payload.Status,
      addresses: (payload.Answer ?? [])
        .filter((answer) => answer.type === 1)
        .map((answer) => answer.data),
    };
  } catch (error) {
    return { ok: false, elapsedMs: elapsedMs(startNs), error: errorInfo(error) };
  }
}

async function probeTls(host, port) {
  const startNs = process.hrtime.bigint();
  return new Promise((resolveProbe) => {
    const socket = tls.connect({ host, port, servername: host, rejectUnauthorized: true });
    let finished = false;
    const finish = (result) => {
      if (finished) return;
      finished = true;
      socket.destroy();
      resolveProbe({ ...result, elapsedMs: elapsedMs(startNs) });
    };
    socket.setTimeout(timeoutMs, () => finish({ ok: false, stage: "timeout" }));
    socket.once("secureConnect", () => {
      const peer = socket.getPeerCertificate();
      finish({
        ok: true,
        authorized: socket.authorized,
        protocol: socket.getProtocol(),
        cipher: socket.getCipher()?.name ?? null,
        peerSubject: peer?.subject?.CN ?? null,
        remoteAddress: socket.remoteAddress ?? null,
      });
    });
    socket.once("error", (error) => finish({ ok: false, stage: "tls", error: errorInfo(error) }));
  });
}

async function probeHttp(url) {
  const startNs = process.hrtime.bigint();
  try {
    const response = await fetch(url, {
      headers: { accept: "application/json,text/plain,*/*", "user-agent": "latency-benchmark-diagnostic/0.1" },
      signal: AbortSignal.timeout(timeoutMs),
    });
    const body = await response.text();
    return {
      ok: response.ok,
      elapsedMs: elapsedMs(startNs),
      status: response.status,
      contentType: response.headers.get("content-type"),
      bodyPrefix: body.slice(0, 240),
    };
  } catch (error) {
    return { ok: false, elapsedMs: elapsedMs(startNs), error: errorInfo(error) };
  }
}

async function probeWebSocket(url) {
  const startNs = process.hrtime.bigint();
  return new Promise((resolveProbe) => {
    const ws = new WebSocket(url);
    let finished = false;
    const finish = (result) => {
      if (finished) return;
      finished = true;
      clearTimeout(timer);
      try { ws.close(1000, "diagnostic complete"); } catch { /* best effort */ }
      resolveProbe({ ...result, elapsedMs: elapsedMs(startNs) });
    };
    const timer = setTimeout(() => finish({ ok: false, stage: "timeout" }), timeoutMs);
    ws.addEventListener("open", () => finish({ ok: true, stage: "open" }));
    ws.addEventListener("error", () => finish({ ok: false, stage: "websocket_error" }));
    ws.addEventListener("close", (event) => finish({
      ok: false,
      stage: "closed_before_open",
      code: event.code,
      reason: event.reason,
    }));
  });
}

const report = {
  schemaVersion: 1,
  generatedAt: new Date().toISOString(),
  runtime: { node: process.version, platform: process.platform, arch: process.arch },
  proxyEnvironment: Object.fromEntries([
    "HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "NO_PROXY",
    "http_proxy", "https_proxy", "all_proxy", "no_proxy",
  ].map((name) => [name, proxyInfo(name)]).filter(([, value]) => value !== null)),
  timeoutMs,
  targets: [],
};

for (const target of targets) {
  const result = {
    id: target.id,
    host: target.host,
    port: target.port,
    dns: await probeDns(target.host),
    dnsOverHttps: await probeDnsOverHttps(target.host),
  };
  if (!args["dns-only"]) {
    result.tls = await probeTls(target.host, target.port);
    if (target.http) result.http = await probeHttp(target.http);
    if (target.websocket) result.websocket = await probeWebSocket(target.websocket);
  }
  report.targets.push(result);
}

const serialized = `${JSON.stringify(report, null, 2)}\n`;
if (args.output) {
  const output = resolve(String(args.output));
  await mkdir(dirname(output), { recursive: true });
  await writeFile(output, serialized);
  console.log(JSON.stringify({ output, targets: report.targets.length }, null, 2));
} else {
  process.stdout.write(serialized);
}
