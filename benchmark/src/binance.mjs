import { normalizeEpochMs } from "./common.mjs";
import { runWebSocketCollector } from "./ws-collector.mjs";

export function binanceEndpoint(symbol, baseUrl = process.env.BINANCE_WS_BASE ?? "wss://data-stream.binance.vision") {
  const lower = symbol.toLowerCase();
  const streams = [
    `${lower}@trade`,
    `${lower}@depth@100ms`,
    `${lower}@bookTicker`,
  ];
  return `${baseUrl.replace(/\/$/, "")}/stream?streams=${streams.join("/")}&timeUnit=MICROSECOND`;
}

export function parseBinanceMessage({ sessionId, symbol }) {
  return (combined, clock) => {
    const payload = combined?.data ?? combined;
    if (!payload || typeof payload !== "object") return [];
    if (combined?.result === null) return [];

    const sourceEventTsMs = normalizeEpochMs(payload.E);
    const sourceTradeTsMs = normalizeEpochMs(payload.T);
    const stream = combined?.stream ?? payload.e ?? "unknown";
    let sequenceStart = null;
    let sequenceEnd = null;
    if (payload.e === "trade") {
      sequenceStart = payload.t ?? null;
      sequenceEnd = payload.t ?? null;
    } else if (payload.e === "depthUpdate") {
      sequenceStart = payload.U ?? null;
      sequenceEnd = payload.u ?? null;
    }

    return [{
      schema_version: 1,
      record_kind: "market_data",
      session_id: sessionId,
      source: "binance",
      stream,
      instrument: payload.s ?? symbol,
      event_type: payload.e ?? (stream.endsWith("@bookTicker") ? "bookTicker" : "unknown"),
      source_event_ts_ms: sourceEventTsMs,
      source_trade_ts_ms: sourceTradeTsMs,
      recv_wall_ts_ms: clock.wallMs,
      recv_mono_ns: clock.monoNs,
      arrival_latency_ms: sourceEventTsMs === null ? null : clock.wallMs - sourceEventTsMs,
      sequence_start: sequenceStart,
      sequence_end: sequenceEnd,
      payload,
    }];
  };
}

export async function collectBinance({ sessionId, symbol, durationMs, writer, stopState, baseUrl, lookup }) {
  const endpoint = binanceEndpoint(symbol, baseUrl);
  await runWebSocketCollector({
    sessionId,
    source: "binance",
    stream: symbol.toLowerCase(),
    endpoint,
    durationMs,
    writer,
    lookup,
    parsePayload: parseBinanceMessage({ sessionId, symbol }),
    stopState,
  });
}
