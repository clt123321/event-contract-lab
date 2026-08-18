# Public market-data latency benchmark

This directory contains read-only collectors for the first local data-source
spike. It does not use account credentials and cannot place orders.

Supported in v0.1:

- Binance Spot public market-data WebSocket: trades, depth deltas and BBO.
- Polymarket public Gamma discovery API and Market Channel WebSocket.
- NDJSON capture with wall-clock and monotonic receive timestamps.
- Basic arrival-latency, sequence-gap, reconnect and parse-error summaries.

Not supported yet:

- Chainlink Data Streams, which requires a separately authorized account.
- Predict.fun, until its public data contract and permitted access path are
  documented.
- Order submission or authenticated user streams.

## Quick start

Requires Node.js 22 or newer. Install the locked runtime dependency once with
`npm install`.

```bash
cd benchmark
npm install

# Inspect candidate Polymarket markets and token IDs.
npm run discover:polymarket -- --query bitcoin --limit 10

# Estimate server-minus-local clock offset before collecting.
npm run probe:clock -- --samples 10

# Run both public collectors for 60 seconds. The Polymarket side automatically
# selects the first active order-book market matching the query.
npm run collect:public -- \
  --duration 60 \
  --symbol BTCUSDT \
  --polymarket-query bitcoin

# Summarize the NDJSON file printed by the collector. Use the clock probe's
# recommendedClockOffsetMs so the report includes corrected distributions.
npm run summarize -- \
  --input data/raw/<session>.ndjson \
  --clock-offset-ms <recommendedClockOffsetMs>

# Produce a layered DNS/TLS/HTTP/WebSocket connectivity report.
npm run diagnose:network -- \
  --timeout 8000 \
  --output data/diagnostics/network.json

# Fast system-DNS versus encrypted-DNS comparison only.
npm run diagnose:network -- \
  --dns-only \
  --output data/diagnostics/dns.json
```

Individual collectors are also available:

```bash
npm run collect:binance -- --duration 60 --symbol BTCUSDT

# If the market-data-only host is unavailable from the current network:
npm run collect:binance -- \
  --duration 60 \
  --symbol BTCUSDT \
  --base-url wss://stream.binance.com:9443

npm run collect:polymarket -- \
  --duration 60 \
  --asset-ids <yes_token_id>,<no_token_id>
```

By default, files are written under `data/raw/`, which is intentionally ignored
by Git. Use `--output /absolute/or/relative/path.ndjson` to choose a file.
The Polymarket collector sends the protocol-required `PING` every 10 seconds.
Collectors default to `--dns doh` because the current network was observed
returning incorrect addresses for several target domains. Use `--dns system`
only on a network whose resolver passes `npm run diagnose:network -- --dns-only`.

## Event envelope

Every line is a JSON object. Market messages include:

```text
schema_version
session_id
source
stream
instrument
event_type
source_event_ts_ms
source_trade_ts_ms
recv_wall_ts_ms
recv_mono_ns
arrival_latency_ms
sequence_start
sequence_end
payload
```

`arrival_latency_ms` is `recv_wall_ts_ms - source_event_ts_ms`. It is useful as
an initial sanity check, but it is not a production one-way latency measurement
until the local clock offset is recorded and bounded. Intra-process durations
must use `recv_mono_ns` or another monotonic timestamp.

The clock probe estimates `server time - local time` with an NTP-style midpoint
calculation and recommends the sample with the lowest round-trip time. The
summary adds that value to raw arrival latency. Keep both values: the corrected
number is still an estimate, not a synchronized-hardware measurement.

Binance `bookTicker` messages do not carry a source timestamp, so they are used
for receive-rate/BBO analysis rather than one-way latency. Polymarket initial
`book` timestamps are reported separately as `snapshot_age_ms`; they are not
mixed into live-event arrival latency.

## What this benchmark does and does not prove

The first run answers:

- Can the public feed be connected and decoded?
- Do messages carry usable source timestamps and sequence identifiers?
- What are the initial P50/P95/P99 arrival-latency distributions?
- Are there reconnects, parse failures or obvious sequence gaps?

It does not yet measure strategy-to-order latency, exchange acknowledgement,
matching, fill confirmation or warehouse availability. Those require additional
instrumentation points and, for order paths, a separately approved canary.

## Official protocol references

- Binance Spot WebSocket streams:
  https://developers.binance.com/en/docs/binance-spot-api-docs/web-socket-streams
- Polymarket Market Channel:
  https://docs.polymarket.com/market-data/websocket/market-channel
- Polymarket public API quickstart:
  https://docs.polymarket.com/quickstart
