export function check(id, passed, actual, expected, severity = "error") {
  return {
    id,
    passed: Boolean(passed),
    severity,
    actual: actual === undefined ? null : actual,
    expected: expected === undefined ? null : expected,
  };
}

export function evaluateRuntime({ nodeVersion, freeDiskMib }, profile) {
  const nodeMajor = Number(String(nodeVersion).replace(/^v/, "").split(".")[0]);
  return [
    check(
      "runtime.node_major",
      Number.isInteger(nodeMajor) && nodeMajor >= profile.minimum_node_major,
      nodeMajor,
      `>= ${profile.minimum_node_major}`,
    ),
    check(
      "runtime.free_disk_mib",
      Number.isFinite(freeDiskMib) && freeDiskMib >= profile.minimum_free_disk_mib,
      Math.floor(freeDiskMib),
      `>= ${profile.minimum_free_disk_mib}`,
    ),
  ];
}

export function evaluateWalImport(importResult, verifyResult, expectedRows) {
  return [
    check("wal.import_status", importResult?.status === "sealed", importResult?.status, "sealed"),
    check("wal.imported_rows", importResult?.imported_rows === expectedRows, importResult?.imported_rows, expectedRows),
    check("wal.verify_status", verifyResult?.status === "verified", verifyResult?.status, "verified"),
    check("wal.verified_rows", verifyResult?.rows === expectedRows, verifyResult?.rows, expectedRows),
    check("wal.segment_count", verifyResult?.segments > 0, verifyResult?.segments, "> 0"),
  ];
}

export function evaluateNormalization(result, quality, manifest, expected, gitCommit) {
  const summary = quality?.summary ?? {};
  const rowsBalance = Number(summary.canonical_rows ?? 0)
    + Number(summary.quarantined_rows ?? 0)
    + Number(summary.skipped_rows ?? 0);
  return [
    check("normalization.status", result?.status === "normalized", result?.status, "normalized"),
    check("normalization.input_rows", summary.input_rows === expected.inputRows, summary.input_rows, expected.inputRows),
    check(
      "normalization.canonical_rows",
      summary.canonical_rows === expected.canonicalRows,
      summary.canonical_rows,
      expected.canonicalRows,
    ),
    check(
      "normalization.quarantined_rows",
      summary.quarantined_rows === expected.quarantinedRows,
      summary.quarantined_rows,
      expected.quarantinedRows,
    ),
    check(
      "normalization.warning_rows",
      summary.warning_rows === expected.warningRows,
      summary.warning_rows,
      expected.warningRows,
    ),
    check(
      "normalization.skipped_rows",
      summary.skipped_rows === expected.skippedRows,
      summary.skipped_rows,
      expected.skippedRows,
    ),
    check("normalization.row_balance", rowsBalance === summary.input_rows, rowsBalance, summary.input_rows),
    check(
      "normalization.input_lineage",
      quality?.input_sha256 === manifest?.input?.sha256,
      quality?.input_sha256,
      manifest?.input?.sha256,
    ),
    check(
      "normalization.policy_binding",
      quality?.quality_policy_sha256 === manifest?.quality_policy_sha256,
      quality?.quality_policy_sha256,
      manifest?.quality_policy_sha256,
    ),
    check("normalization.code_commit", manifest?.code_commit === gitCommit, manifest?.code_commit, gitCommit),
    check(
      "normalization.transform_id",
      /^[a-f0-9]{64}$/.test(String(manifest?.transform_id ?? "")),
      manifest?.transform_id,
      "64 lowercase hex characters",
    ),
  ];
}

export function evaluateHostNormalization(result, quality, manifest, profile, gitCommit) {
  const summary = quality?.summary ?? {};
  const inputRows = Number(summary.input_rows ?? 0);
  const canonicalRows = Number(summary.canonical_rows ?? 0);
  const quarantinedRows = Number(summary.quarantined_rows ?? 0);
  const skippedRows = Number(summary.skipped_rows ?? 0);
  const quarantineRatio = inputRows > 0 ? quarantinedRows / inputRows : null;
  return [
    check("host_normalization.status", result?.status === "normalized", result?.status, "normalized"),
    check(
      "host_normalization.row_balance",
      canonicalRows + quarantinedRows + skippedRows === inputRows,
      canonicalRows + quarantinedRows + skippedRows,
      inputRows,
    ),
    check(
      "host_normalization.canonical_rows",
      canonicalRows >= profile.minimum_canonical_rows,
      canonicalRows,
      `>= ${profile.minimum_canonical_rows}`,
    ),
    check(
      "host_normalization.quarantine_ratio",
      quarantineRatio !== null && quarantineRatio <= profile.maximum_quarantine_ratio,
      quarantineRatio,
      `<= ${profile.maximum_quarantine_ratio}`,
    ),
    check(
      "host_normalization.input_lineage",
      quality?.input_sha256 === manifest?.input?.sha256,
      quality?.input_sha256,
      manifest?.input?.sha256,
    ),
    check(
      "host_normalization.policy_binding",
      quality?.quality_policy_sha256 === manifest?.quality_policy_sha256,
      quality?.quality_policy_sha256,
      manifest?.quality_policy_sha256,
    ),
    check(
      "host_normalization.code_commit",
      manifest?.code_commit === gitCommit,
      manifest?.code_commit,
      gitCommit,
    ),
  ];
}

export function evaluateDatasetReplay(datasetResult, dataset, replayResult, replay, profile, gitCommit) {
  return [
    check("dataset.status", datasetResult?.status === "frozen", datasetResult?.status, "frozen"),
    check("dataset.input_rows", dataset?.input_rows === profile.dataset_input_rows, dataset?.input_rows, profile.dataset_input_rows),
    check("dataset.included_rows", dataset?.included_rows === profile.dataset_included_rows, dataset?.included_rows, profile.dataset_included_rows),
    check("dataset.excluded_rows", dataset?.excluded_rows === profile.dataset_excluded_rows, dataset?.excluded_rows, profile.dataset_excluded_rows),
    check(
      "dataset.row_balance",
      Number(dataset?.included_rows ?? 0) + Number(dataset?.excluded_rows ?? 0) === dataset?.input_rows,
      Number(dataset?.included_rows ?? 0) + Number(dataset?.excluded_rows ?? 0),
      dataset?.input_rows,
    ),
    check("dataset.code_commit", dataset?.code_commit === gitCommit, dataset?.code_commit, gitCommit),
    check(
      "dataset.parquet_sha256",
      /^[a-f0-9]{64}$/.test(String(dataset?.parquet_files?.[0]?.sha256 ?? "")),
      dataset?.parquet_files?.[0]?.sha256,
      "64 lowercase hex characters",
    ),
    check("replay.status", replayResult?.status === "replayed", replayResult?.status, "replayed"),
    check("replay.dataset_binding", replay?.dataset_id === dataset?.dataset_id, replay?.dataset_id, dataset?.dataset_id),
    check("replay.event_count", replay?.event_count === profile.replay_event_rows, replay?.event_count, profile.replay_event_rows),
    check("replay.output_rows", replay?.output?.row_count === replay?.event_count, replay?.output?.row_count, replay?.event_count),
    check("replay.code_commit", replay?.code_commit === gitCommit, replay?.code_commit, gitCommit),
  ];
}

export function evaluateHostDatasetReplay(datasetResult, dataset, replayResult, replay, gitCommit) {
  return [
    check("host_dataset.status", datasetResult?.status === "frozen", datasetResult?.status, "frozen"),
    check("host_dataset.nonempty", dataset?.included_rows > 0, dataset?.included_rows, "> 0"),
    check(
      "host_dataset.row_balance",
      Number(dataset?.included_rows ?? 0) + Number(dataset?.excluded_rows ?? 0) === dataset?.input_rows,
      Number(dataset?.included_rows ?? 0) + Number(dataset?.excluded_rows ?? 0),
      dataset?.input_rows,
    ),
    check("host_dataset.code_commit", dataset?.code_commit === gitCommit, dataset?.code_commit, gitCommit),
    check("host_replay.status", replayResult?.status === "replayed", replayResult?.status, "replayed"),
    check("host_replay.dataset_binding", replay?.dataset_id === dataset?.dataset_id, replay?.dataset_id, dataset?.dataset_id),
    check("host_replay.event_count", replay?.event_count === dataset?.included_rows, replay?.event_count, dataset?.included_rows),
    check("host_replay.output_rows", replay?.output?.row_count === replay?.event_count, replay?.output?.row_count, replay?.event_count),
  ];
}

export function evaluateClock(clockReport, profile) {
  const offset = clockReport?.recommendedClockOffsetMs;
  return [
    check(
      "clock.samples",
      clockReport?.samples >= profile.clock_samples,
      clockReport?.samples,
      `>= ${profile.clock_samples}`,
    ),
    check(
      "clock.absolute_offset_ms",
      Number.isFinite(offset) && Math.abs(offset) <= profile.maximum_absolute_clock_offset_ms,
      Number.isFinite(offset) ? Math.abs(offset) : offset,
      `<= ${profile.maximum_absolute_clock_offset_ms}`,
    ),
  ];
}

function connectionTotals(summary) {
  return Object.values(summary?.connections ?? {}).reduce((totals, item) => ({
    errors: totals.errors + Number(item?.errors ?? 0),
    parseErrors: totals.parseErrors + Number(item?.parseErrors ?? 0),
  }), { errors: 0, parseErrors: 0 });
}

export function evaluateCaptureSummary(summary, profile) {
  const totals = connectionTotals(summary);
  const groupKeys = Object.keys(summary?.groups ?? {});
  const presentSources = new Set(groupKeys.map((key) => key.split("|")[0]));
  const checks = [
    check("capture.rows", summary?.rows >= profile.minimum_capture_rows, summary?.rows, `>= ${profile.minimum_capture_rows}`),
    check("capture.invalid_json", summary?.invalidJson <= profile.maximum_invalid_json, summary?.invalidJson, `<= ${profile.maximum_invalid_json}`),
    check("capture.connection_errors", totals.errors <= profile.maximum_connection_errors, totals.errors, `<= ${profile.maximum_connection_errors}`),
    check("capture.parse_errors", totals.parseErrors <= profile.maximum_parse_errors, totals.parseErrors, `<= ${profile.maximum_parse_errors}`),
  ];
  for (const source of profile.required_market_data_sources) {
    checks.push(check(
      `capture.source.${source}`,
      presentSources.has(source),
      presentSources.has(source),
      true,
    ));
  }
  return checks;
}

export function evaluateNetwork(networkReport, profile) {
  const byId = new Map((networkReport?.targets ?? []).map((target) => [target.id, target]));
  return profile.required_network_targets.flatMap((id) => {
    const target = byId.get(id);
    const protocolResult = target?.websocket ?? target?.http ?? null;
    return [
      check(`network.${id}.present`, Boolean(target), Boolean(target), true),
      check(`network.${id}.doh`, target?.dnsOverHttps?.ok === true, target?.dnsOverHttps?.ok, true),
      check(`network.${id}.protocol`, protocolResult?.ok === true, protocolResult?.ok, true),
    ];
  });
}

export function overallStatus(checks) {
  if (checks.some((item) => item.severity === "error" && !item.passed)) return "failed";
  if (checks.some((item) => !item.passed)) return "warning";
  return "passed";
}

export function compareReports(before, after) {
  const beforeChecks = new Map((before?.checks ?? []).map((item) => [item.id, item]));
  const afterChecks = new Map((after?.checks ?? []).map((item) => [item.id, item]));
  const checkIds = [...new Set([...beforeChecks.keys(), ...afterChecks.keys()])].sort();
  const transitions = checkIds.map((id) => {
    const previous = beforeChecks.get(id) ?? null;
    const current = afterChecks.get(id) ?? null;
    let change = "unchanged";
    if (previous?.passed === false && current?.passed === true) change = "improved";
    if (previous?.passed === true && current?.passed === false) change = "regressed";
    if (previous === null) change = "added";
    if (current === null) change = "removed";
    return { id, change, before: previous, after: current };
  });

  const beforeSteps = new Map((before?.steps ?? []).map((item) => [item.id, item]));
  const afterSteps = new Map((after?.steps ?? []).map((item) => [item.id, item]));
  const durationChanges = [...afterSteps.entries()].map(([id, current]) => {
    const previous = beforeSteps.get(id);
    const beforeMs = previous?.duration_ms ?? null;
    return {
      id,
      before_ms: beforeMs,
      after_ms: current.duration_ms,
      delta_ms: beforeMs === null ? null : current.duration_ms - beforeMs,
    };
  });

  return {
    schema_version: 1,
    before: { run_id: before?.run_id, status: before?.status, commit: before?.git?.commit },
    after: { run_id: after?.run_id, status: after?.status, commit: after?.git?.commit },
    same_verification_profile: before?.verification_profile?.sha256 === after?.verification_profile?.sha256,
    same_market_universe: before?.configuration?.market_universe?.sha256
      === after?.configuration?.market_universe?.sha256,
    improved: transitions.filter((item) => item.change === "improved"),
    regressed: transitions.filter((item) => item.change === "regressed"),
    unresolved: transitions.filter((item) => item.after?.passed === false),
    transitions,
    duration_changes: durationChanges,
  };
}
