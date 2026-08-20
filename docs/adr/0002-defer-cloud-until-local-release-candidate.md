# ADR-0002: Defer cloud application until the local release candidate passes

- Status: Accepted
- Date: 2026-08-20
- Decision owner: project owner

## Context

Most contracts, persistence, replay, paper execution, failure handling, packaging, IaC planning, and
verification logic do not require a cloud account. Applying for a server too early adds cost and creates
pressure to debug manually on a mutable host. Some evidence—regional routing, host clock behavior,
long-running capacity and the actual bill—can only be collected after deployment.

## Decision

Complete as much implementation and failure testing locally as possible. A clean commit must pass the
versioned local release verification before requesting the approved 14-day Tokyo benchmark resources.
The future host runs the same verification entry point and report schema immediately after deployment.

Cloud remains necessary only for the residual evidence that local development cannot establish. The
approved $150 ceiling remains valid, but it does not trigger account or resource creation by itself.

## Consequences

- Development of schemas, WAL/Parquet, canonical models, replay, paper OMS, fault injection, packaging,
  Terraform plan, preflight, rollback, and verification proceeds without waiting for AWS.
- `make verify-local` supports the development loop; `make verify-release` is the clean L3 gate; future
  hosts start with `make verify-host`.
- Every verification run has a new directory, immutable report, step logs, commit and parameters.
- Dynamic Polymarket discovery is permitted only for connectivity smoke and is marked non-formal.
- The cloud benchmark starts later but should spend less time on avoidable application/debugging work.

## Alternatives considered

- Applying for AWS immediately was rejected because it does not unblock current local engineering.
- Eliminating the cloud benchmark was rejected because local tests cannot prove Tokyo routing, clock,
  sustained capacity, cloud operational behavior, or real resource cost.

## Rollback

If a source becomes unreachable locally and blocks contract discovery, a narrowly scoped temporary host
may be proposed as a separate exception. It still requires a budget owner, expiry time, read-only scope,
and a verification report; it does not relax G3.
