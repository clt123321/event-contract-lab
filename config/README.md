# Versioned runtime scope

`market-universe.json` is the reviewable source of truth for the first data scope. It deliberately
contains no secret, wallet, account identifier, or live-write setting.

The empty Predict.fun and Polymarket market ID lists are not implementation gaps to hide with an
automatic fallback. Predict.fun IDs must come from an authorized Testnet contract. Polymarket's
3–5 markets must be discovered through the public tool, reviewed for rules/end time/liquidity, and
then frozen in a pull request. A dynamic “top market” result is never a formal benchmark universe.

Any future change that enables a write environment requires a separate G3 decision and must also
change the CI safety policy. A configuration-only change cannot enable live execution.
