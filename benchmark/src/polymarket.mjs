import { normalizeEpochMs } from "./common.mjs";
import { requestJson } from "./network.mjs";
import { runWebSocketCollector } from "./ws-collector.mjs";

export const POLYMARKET_WS = "wss://ws-subscriptions-clob.polymarket.com/ws/market";
export const POLYMARKET_MARKETS = "https://gamma-api.polymarket.com/markets";
export const POLYMARKET_CLOB_BOOK = "https://clob.polymarket.com/book";
export const POLYMARKET_DATA_TRADES = "https://data-api.polymarket.com/trades";

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
      conditionId: market.conditionId ?? null,
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

export async function fetchPolymarketPublicSnapshot({ market, lookup, tradeLimit = 100 } = {}) {
  if (!market?.assetIds?.length) throw new Error("market with at least one asset ID is required");

  const orderbooks = await Promise.all(market.assetIds.map(async (assetId) => {
    const url = new URL(POLYMARKET_CLOB_BOOK);
    url.searchParams.set("token_id", String(assetId));
    return requestJson(url, { lookup });
  }));

  let trades = [];
  if (market.conditionId) {
    const url = new URL(POLYMARKET_DATA_TRADES);
    url.searchParams.set("market", String(market.conditionId));
    url.searchParams.set("limit", String(tradeLimit));
    trades = await requestJson(url, { lookup });
  }

  return { market, orderbooks, trades };
}

export function polymarketSnapshotRecords({ sessionId, snapshot, clock }) {
  const base = {
    schema_version: 1,
    session_id: sessionId,
    source: "polymarket",
    recv_wall_ts_ms: clock.wallMs,
    recv_mono_ns: clock.monoNs,
    sequence_start: null,
    sequence_end: null,
  };
  const records = [{
    ...base,
    record_kind: "market_metadata",
    stream: "gamma_market",
    instrument: snapshot.market.slug ?? String(snapshot.market.id),
    event_type: "market_snapshot",
    source_event_ts_ms: null,
    source_trade_ts_ms: null,
    arrival_latency_ms: null,
    snapshot_age_ms: null,
    payload: snapshot.market,
  }];

  for (const book of snapshot.orderbooks ?? []) {
    const sourceEventTsMs = normalizeEpochMs(book.timestamp);
    records.push({
      ...base,
      record_kind: "market_data",
      stream: "clob_book_rest",
      instrument: String(book.asset_id ?? "unknown"),
      asset_id: book.asset_id ?? null,
      market: book.market ?? snapshot.market.conditionId ?? null,
      event_type: "book",
      source_event_ts_ms: sourceEventTsMs,
      source_trade_ts_ms: null,
      arrival_latency_ms: null,
      snapshot_age_ms: sourceEventTsMs === null ? null : clock.wallMs - sourceEventTsMs,
      payload: book,
    });
  }

  for (const trade of snapshot.trades ?? []) {
    const sourceTradeTsMs = normalizeEpochMs(trade.timestamp);
    records.push({
      ...base,
      record_kind: "market_data",
      stream: "data_trades",
      instrument: String(trade.asset ?? trade.conditionId ?? "unknown"),
      asset_id: trade.asset ?? null,
      market: trade.conditionId ?? snapshot.market.conditionId ?? null,
      event_type: "trade",
      source_event_ts_ms: sourceTradeTsMs,
      source_trade_ts_ms: sourceTradeTsMs,
      arrival_latency_ms: null,
      snapshot_age_ms: null,
      payload: trade,
    });
  }

  return records;
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
