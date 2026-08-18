import { messageDataToText, receiveClock, sleep } from "./common.mjs";
import WebSocket from "ws";

function connectionRecord({ sessionId, source, stream, eventType, endpoint, attempt, details }) {
  const clock = receiveClock();
  return {
    schema_version: 1,
    record_kind: "connection",
    session_id: sessionId,
    source,
    stream,
    event_type: eventType,
    endpoint,
    attempt,
    recv_wall_ts_ms: clock.wallMs,
    recv_mono_ns: clock.monoNs,
    details: details ?? null,
  };
}

export async function runWebSocketCollector({
  sessionId,
  source,
  stream,
  endpoint,
  durationMs,
  writer,
  subscribe,
  heartbeat,
  lookup,
  parsePayload,
  stopState,
  maxBackoffMs = 10_000,
}) {
  const deadline = Date.now() + durationMs;
  let attempt = 0;
  let activeSocket = null;
  let backoffMs = 250;

  stopState.closers.add(() => activeSocket?.close(1000, "collector stopping"));

  while (!stopState.stopped && Date.now() < deadline) {
    attempt += 1;
    let opened = false;
    let shouldReconnect = true;

    await new Promise((resolveConnection) => {
      const ws = new WebSocket(endpoint, { lookup, handshakeTimeout: 10_000 });
      activeSocket = ws;
      let deadlineTimer = null;
      let heartbeatTimer = null;
      let finished = false;

      const finish = () => {
        if (finished) return;
        finished = true;
        clearTimeout(deadlineTimer);
        clearInterval(heartbeatTimer);
        if (activeSocket === ws) activeSocket = null;
        resolveConnection();
      };

      ws.addEventListener("open", () => {
        opened = true;
        backoffMs = 250;
        writer.write(connectionRecord({
          sessionId,
          source,
          stream,
          eventType: "connection_open",
          endpoint,
          attempt,
        }));
        if (subscribe) ws.send(JSON.stringify(subscribe));
        if (heartbeat) {
          heartbeatTimer = setInterval(() => {
            if (ws.readyState === WebSocket.OPEN) ws.send(heartbeat.payload);
          }, heartbeat.intervalMs);
          heartbeatTimer.unref?.();
        }
      });

      ws.addEventListener("message", async (event) => {
        const clock = receiveClock();
        try {
          const text = await messageDataToText(event.data);
          if (text === "PONG") return;
          const payload = JSON.parse(text);
          const records = parsePayload(payload, clock);
          for (const record of records) writer.write(record);
        } catch (error) {
          writer.write(connectionRecord({
            sessionId,
            source,
            stream,
            eventType: "parse_error",
            endpoint,
            attempt,
            details: String(error?.message ?? error),
          }));
        }
      });

      ws.addEventListener("error", () => {
        writer.write(connectionRecord({
          sessionId,
          source,
          stream,
          eventType: "connection_error",
          endpoint,
          attempt,
        }));
      });

      ws.addEventListener("close", (event) => {
        shouldReconnect = !stopState.stopped && Date.now() < deadline;
        writer.write(connectionRecord({
          sessionId,
          source,
          stream,
          eventType: "connection_close",
          endpoint,
          attempt,
          details: { code: event.code, reason: event.reason, opened },
        }));
        finish();
      });

      const remainingMs = Math.max(0, deadline - Date.now());
      deadlineTimer = setTimeout(() => {
        shouldReconnect = false;
        ws.close(1000, "duration complete");
      }, remainingMs);
      deadlineTimer.unref?.();
    });

    if (shouldReconnect) {
      await sleep(Math.min(backoffMs, Math.max(0, deadline - Date.now())));
      backoffMs = Math.min(maxBackoffMs, backoffMs * 2);
    }
  }
}

export function createStopState() {
  const state = {
    stopped: false,
    reason: null,
    closers: new Set(),
    stop(reason = "requested") {
      if (state.stopped) return;
      state.stopped = true;
      state.reason = reason;
      for (const close of state.closers) {
        try {
          close();
        } catch {
          // Best effort shutdown.
        }
      }
    },
  };
  return state;
}
