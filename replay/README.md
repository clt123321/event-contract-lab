# Replay (P2 pending)

The replay engine will consume frozen dataset manifests, order events by their point-in-time visibility,
use a virtual clock, and record code/config/data/seed. It must not infer fills from candle touches.

Implementation starts after P1 has a representative 24-hour sample and stable canonical semantics.
