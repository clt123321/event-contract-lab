import https from "node:https";

const addressCache = new Map();

export function dnsModeFromArgs(args) {
  const mode = String(args.dns ?? process.env.MARKET_DATA_DNS ?? "doh").toLowerCase();
  if (!["doh", "system"].includes(mode)) throw new Error("--dns must be doh or system");
  return mode;
}

async function resolveDnsOverHttps(hostname, timeoutMs = 8_000) {
  const cached = addressCache.get(hostname);
  if (cached && cached.expiresAt > Date.now()) return cached.addresses;

  const url = new URL("https://cloudflare-dns.com/dns-query");
  url.searchParams.set("name", hostname);
  url.searchParams.set("type", "A");
  const response = await fetch(url, {
    headers: { accept: "application/dns-json" },
    signal: AbortSignal.timeout(timeoutMs),
  });
  if (!response.ok) throw new Error(`DNS-over-HTTPS failed: HTTP ${response.status}`);
  const payload = await response.json();
  if (payload.Status !== 0) throw new Error(`DNS-over-HTTPS failed: status ${payload.Status}`);
  const answers = (payload.Answer ?? []).filter((answer) => answer.type === 1 && answer.data);
  const addresses = answers.map((answer) => String(answer.data));
  if (addresses.length === 0) throw new Error(`DNS-over-HTTPS returned no A record for ${hostname}`);
  const ttlMs = Math.max(10_000, Math.min(...answers.map((answer) => Number(answer.TTL) * 1_000 || 60_000)));
  addressCache.set(hostname, { addresses, expiresAt: Date.now() + ttlMs, next: 0 });
  return addresses;
}

export function createDnsLookup(mode = "doh") {
  if (mode === "system") return undefined;
  if (mode !== "doh") throw new Error(`Unsupported DNS mode: ${mode}`);

  return (hostname, options, callback) => {
    resolveDnsOverHttps(hostname).then((addresses) => {
      const cached = addressCache.get(hostname);
      const index = cached?.next ?? 0;
      const ordered = addresses.map((_, offset) => addresses[(index + offset) % addresses.length]);
      if (cached) cached.next = (index + 1) % addresses.length;

      const all = typeof options === "object" && options?.all;
      if (all) {
        callback(null, ordered.map((address) => ({ address, family: 4 })));
      } else {
        callback(null, ordered[0], 4);
      }
    }).catch((error) => callback(error));
  };
}

export async function requestJson(url, { lookup, timeoutMs = 15_000, headers = {} } = {}) {
  return new Promise((resolveRequest, rejectRequest) => {
    const request = https.get(url, {
      lookup,
      headers: {
        accept: "application/json",
        "user-agent": "event-contract-latency-benchmark/0.1",
        ...headers,
      },
    }, (response) => {
      response.setEncoding("utf8");
      let body = "";
      response.on("data", (chunk) => { body += chunk; });
      response.on("end", () => {
        const status = response.statusCode ?? 0;
        if (status < 200 || status >= 300) {
          rejectRequest(new Error(`HTTP ${status}: ${body.slice(0, 240)}`));
          return;
        }
        try {
          resolveRequest(JSON.parse(body));
        } catch (error) {
          rejectRequest(new Error(`Invalid JSON response: ${error.message}`));
        }
      });
    });
    request.setTimeout(timeoutMs, () => request.destroy(new Error(`Request timed out after ${timeoutMs} ms`)));
    request.on("error", rejectRequest);
  });
}
