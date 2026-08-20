# Versioned runtime scope

`market-universe.json` is the reviewable source of truth for the first data scope. It deliberately
contains no secret, wallet, account identifier, or live-write setting.

The empty Predict.fun and Polymarket market ID lists are not implementation gaps to hide with an
automatic fallback. Predict.fun IDs must come from an authorized Testnet contract. Polymarket's
3–5 markets must be discovered through the public tool, reviewed for rules/end time/liquidity, and
then frozen in a pull request. A dynamic “top market” result is never a formal benchmark universe.

Any future change that enables a write environment requires a separate G3 decision and must also
change the CI safety policy. A configuration-only change cannot enable live execution.

`quality-policy.v1.json` is the first Raw→Silver acceptance policy. Formal transforms bind its SHA-256;
semantic changes require a new policy version rather than editing evidence for an existing dataset.

`quality-mask.strict-v1.json` is the default Silver→Dataset research mask. It excludes every warning;
purpose-specific relaxations require a new file, version and human approval. `replay.v1.json` freezes the
replay seed and optional event cap. Both files are hashed into their output manifests.
