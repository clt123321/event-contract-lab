import { normalizeEpochMs } from "./common.mjs";
import { requestJson } from "./network.mjs";
import { runWebSocketCollector } from "./ws-collector.mjs";

export const POLYMARKET_WS = "wss://ws-subscriptions-clob.polymarket.com/ws/market";
export const POLYMARKET_MARKETS = "https://gamma-api.polymarket.com/markets";

function parseJsonArray(value) {
  if (Array.isArray(value)) return value;
  if (typeof value !== "string") return [];
  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

export async function discoverPolymarketMarkets({ query = "bitcoin", limit = 200, lookup } = {}) {
  const url = new URL(POLYMARKET_MARKETS);
  url.searchParams.set("active", "true");
  url.searchParams.set("closed", "false");
  url.searchParams.set("limit", String(limit));
  url.searchParams.set("order", "volume24hr");
  url.searchParams.set("ascending", "false");

  const markets = await requestJson(url, { lookup });
  const needle = query.trim().toLowerCase();
  return markets
    .filter((market) => {
      const haystack = `${market.slug ?? ""} ${market.question ?? ""}`.toLowerCase();
      return !needle || haystack.includes(needle);
    })
    .map((market) => ({
      id: market.id,
      slug: market.slug,
      question: market.question,
      endDate: market.endDate,
      volume24hr: market.volume24hr ?? null,
      acceptingOrders: market.acceptingOrders ?? null,
      enableOrderBook: market.enableOrderBook ?? null,
      outcomes: parseJsonArray(market.outcomes),
      assetIds: parseJsonArray(market.clobTokenIds),
    }))
    .filter((market) => market.assetIds.length > 0);
}

export function parsePolymarketMessage({ sessionId, assetLabels = new Map(), allowedAssetIds = null }) {
  return (incoming, clock) => {
    const messages = Array.isArray(incoming) ? incoming : [incoming];
    const records = [];
    for (const payload of messages) {
      if (!payload || typeof payload !== "object") continue;
      const sourceEventTsMs = normalizeEpochMs(payload.timestamp);
      const assetId = payload.asset_id ?? payload.asset_id_hex ?? null;
      if (allowedAssetIds && (assetId === null || !allowedAssetIds.has(String(assetId)))) continue;
      const timestampDeltaMs = sourceEventTsMs === null ? null : clock.wallMs - sourceEventTsMs;
      const isSnapshot = payload.event_type === "book";
      records.push({
        schema_version: 1,
        record_kind: "market_data",
        session_id: sessionId,
        source: "polymarket",
        stream: "market",
        instrument: assetLabels.get(String(assetId)) ?? String(assetId ?? payload.market ?? "unknown"),
        asset_id: assetId,
        market: payload.market ?? null,
        event_type: payload.event_type ?? "unknown",
        source_event_ts_ms: sourceEventTsMs,
        source_trade_ts_ms: null,
        recv_wall_ts_ms: clock.wallMs,
        recv_mono_ns: clock.monoNs,
        arrival_latency_ms: isSnapshot ? null : timestampDeltaMs,
        snapshot_age_ms: isSnapshot ? timestampDeltaMs : null,
        sequence_start: null,
        sequence_end: null,
        payload,
      });
    }
    return records;
  };
}

export async function collectPolymarket({
  sessionId,
  assetIds,
  assetLabels = new Map(),
  durationMs,
  writer,
  stopState,
  lookup,
}) {
  await runWebSocketCollector({
    sessionId,
    source: "polymarket",
    stream: "market",
    endpoint: POLYMARKET_WS,
    durationMs,
    writer,
    lookup,
    subscribe: {
      assets_ids: assetIds,
      type: "market",
      custom_feature_enabled: true,
    },
    heartbeat: { intervalMs: 10_000, payload: "PING" },
    parsePayload: parsePolymarketMessage({
      sessionId,
      assetLabels,
      allowedAssetIds: new Set(assetIds.map(String)),
    }),
    stopState,
  });
}
