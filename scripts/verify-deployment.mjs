#!/usr/bin/env node

import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { access, mkdir, readFile, statfs, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  check,
  evaluateCaptureSummary,
  evaluateClock,
  evaluateNetwork,
  evaluateRuntime,
  evaluateWalImport,
  overallStatus,
} from "./lib/verification.mjs";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const profilePath = join(repositoryRoot, "config/verification-profile.json");
const profileText = await readFile(profilePath, "utf8");
const profile = JSON.parse(profileText);
const profileSha256 = createHash("sha256").update(profileText).digest("hex");
const marketUniversePath = join(repositoryRoot, "config/market-universe.json");
const marketUniverseText = await readFile(marketUniversePath, "utf8");
const marketUniverse = JSON.parse(marketUniverseText);
const marketUniverseSha256 = createHash("sha256").update(marketUniverseText).digest("hex");

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const item = argv[index];
    if (!item.startsWith("--")) throw new Error(`Unexpected positional argument: ${item}`);
    const key = item.slice(2);
    const next = argv[index + 1];
    if (next === undefined || next.startsWith("--")) {
      parsed[key] = true;
    } else {
      parsed[key] = next;
      index += 1;
    }
  }
  return parsed;
}

function positiveNumber(value, fallback, name) {
  if (value === undefined) return fallback;
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed <= 0) throw new Error(`${name} must be positive`);
  return parsed;
}

function safeRunId(mode) {
  const timestamp = new Date().toISOString().replace(/[-:.]/g, "").replace("Z", "Z");
  const entropy = Math.random().toString(16).slice(2, 8);
  return `${mode}-${timestamp}-${entropy}`;
}

async function createOutputDirectory(requested, runId) {
  const output = requested
    ? resolve(repositoryRoot, String(requested))
    : join(repositoryRoot, "data", "verification", runId);
  await mkdir(dirname(output), { recursive: true });
  await mkdir(output);
  await mkdir(join(output, "logs"));
  return output;
}

function parseJsonOutput(step) {
  try {
    return JSON.parse(step.stdout.trim());
  } catch (error) {
    throw new Error(`Step ${step.id} did not emit one JSON document: ${error.message}`);
  }
}

const args = parseArgs(process.argv.slice(2));
const mode = String(args.mode ?? "local");
if (!new Set(["local", "host-smoke"]).has(mode)) {
  throw new Error("--mode must be local or host-smoke");
}

const runId = safeRunId(mode);
const outputDirectory = await createOutputDirectory(args.output, runId);
const startedAt = new Date().toISOString();
const steps = [];
const checks = [];

async function runStep(id, command, commandArgs, { required = true, severity = required ? "error" : "warning" } = {}) {
  const startedNs = process.hrtime.bigint();
  const result = await new Promise((resolveRun) => {
    const child = spawn(command, commandArgs, {
      cwd: repositoryRoot,
      env: process.env,
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => { stdout += chunk; });
    child.stderr.on("data", (chunk) => { stderr += chunk; });
    child.once("error", (error) => resolveRun({ exitCode: null, stdout, stderr: `${stderr}${error.message}\n` }));
    child.once("close", (exitCode) => resolveRun({ exitCode, stdout, stderr }));
  });
  const durationMs = Number(process.hrtime.bigint() - startedNs) / 1e6;
  const stdoutLog = join(outputDirectory, "logs", `${id}.stdout.log`);
  const stderrLog = join(outputDirectory, "logs", `${id}.stderr.log`);
  await Promise.all([
    writeFile(stdoutLog, result.stdout),
    writeFile(stderrLog, result.stderr),
  ]);
  const step = {
    id,
    command,
    args: commandArgs,
    exit_code: result.exitCode,
    duration_ms: Math.round(durationMs * 1000) / 1000,
    stdout_log: stdoutLog,
    stderr_log: stderrLog,
    stdout: result.stdout,
  };
  steps.push(step);
  checks.push(check(`step.${id}`, result.exitCode === 0, result.exitCode, 0, severity));
  if (required && result.exitCode !== 0) throw new Error(`Required step failed: ${id}`);
  return step;
}

async function gitMetadata() {
  const commitStep = await runStep("git-commit", "git", ["rev-parse", "HEAD"]);
  const statusStep = await runStep("git-status", "git", ["status", "--porcelain"]);
  const dirty = statusStep.stdout.trim().length > 0;
  checks.push(check(
    "git.clean",
    !dirty,
    dirty ? "dirty" : "clean",
    "clean",
    args["require-clean"] ? "error" : "warning",
  ));
  return { commit: commitStep.stdout.trim(), dirty };
}

async function runtimeChecks() {
  const filesystem = await statfs(repositoryRoot, { bigint: true });
  const freeDiskMib = Number(filesystem.bavail * filesystem.bsize) / (1024 * 1024);
  checks.push(...evaluateRuntime({ nodeVersion: process.version, freeDiskMib }, profile.local));
  return {
    node: process.version,
    platform: process.platform,
    arch: process.arch,
    free_disk_mib: Math.floor(freeDiskMib),
  };
}

async function ensureWalBinary() {
  const binary = join(repositoryRoot, "target", "debug", process.platform === "win32" ? "wal-cli.exe" : "wal-cli");
  if (args["no-build"]) {
    await access(binary);
  } else {
    await runStep("wal-build", "cargo", ["build", "--locked", "-p", "wal-cli"]);
  }
  return binary;
}

async function runLocalVerification(walBinary, git) {
  if (!args["skip-checks"]) {
    await runStep("rust-format", "cargo", ["fmt", "--all", "--", "--check"]);
    await runStep("rust-clippy", "cargo", ["clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"]);
    await runStep("rust-test", "cargo", ["test", "--workspace", "--locked"]);
    await runStep("node-test", "npm", ["--prefix", "benchmark", "test"]);
  }

  const walDirectory = join(outputDirectory, "wal");
  const fixture = join(repositoryRoot, "fixtures", "raw", "sample-events.v1.ndjson");
  const importStep = await runStep("fixture-wal-import", walBinary, [
    "import", "--input", fixture,
    "--wal-dir", walDirectory,
    "--max-segment-bytes", String(profile.local.fixture_segment_bytes),
    "--git-commit", git.commit,
  ]);
  const verifyStep = await runStep("fixture-wal-verify", walBinary, ["verify", "--wal-dir", walDirectory]);
  checks.push(...evaluateWalImport(
    parseJsonOutput(importStep),
    parseJsonOutput(verifyStep),
    profile.local.fixture_rows,
  ));
}

async function runHostSmoke(walBinary, git) {
  const hostProfile = profile.host_smoke;
  const duration = positiveNumber(args.duration, hostProfile.duration_seconds, "--duration");
  const dnsMode = String(args.dns ?? "doh");
  const symbol = String(args.symbol ?? "BTCUSDT").toUpperCase();
  const polymarketQuery = String(args["polymarket-query"] ?? "bitcoin");
  const networkPath = join(outputDirectory, "network.json");
  const capturePath = join(outputDirectory, "capture.ndjson");
  const walDirectory = join(outputDirectory, "wal");

  const [networkStep, clockStep, captureStep] = await Promise.all([
    runStep("network", process.execPath, [
      "benchmark/src/diagnose-network.mjs",
      "--timeout", String(hostProfile.network_timeout_ms),
      "--output", networkPath,
    ], { required: false, severity: "error" }),
    runStep("clock", process.execPath, [
      "benchmark/src/probe-clock.mjs",
      "--samples", String(hostProfile.clock_samples),
      "--dns", dnsMode,
    ], { required: false, severity: "error" }),
    runStep("capture", process.execPath, [
      "benchmark/src/collect-public.mjs",
      "--duration", String(duration),
      "--symbol", symbol,
      "--polymarket-query", polymarketQuery,
      "--dns", dnsMode,
      "--output", capturePath,
    ], { required: false, severity: "error" }),
  ]);
  let clockReport = null;
  if (clockStep.exit_code === 0) {
    try {
      clockReport = parseJsonOutput(clockStep);
    } catch (error) {
      checks.push(check("clock.report_json", false, String(error.message), "valid JSON"));
    }
  }
  if (networkStep.exit_code === 0) {
    try {
      const network = JSON.parse(await readFile(networkPath, "utf8"));
      checks.push(...evaluateNetwork(network, hostProfile));
    } catch (error) {
      checks.push(check("network.report_json", false, String(error.message), "valid JSON"));
    }
  }
  checks.push(...evaluateClock(clockReport, hostProfile));

  if (captureStep.exit_code === 0) {
    let captureResult = null;
    try {
      captureResult = parseJsonOutput(captureStep);
    } catch (error) {
      checks.push(check("capture.report_json", false, String(error.message), "valid JSON"));
    }
    const summaryArgs = ["benchmark/src/summarize.mjs", "--input", capturePath];
    if (Number.isFinite(clockReport?.recommendedClockOffsetMs)) {
      summaryArgs.push("--clock-offset-ms", String(clockReport.recommendedClockOffsetMs));
    }
    const summaryStep = await runStep("summary", process.execPath, summaryArgs, { required: false, severity: "error" });
    const importStep = await runStep("capture-wal-import", walBinary, [
      "import", "--input", capturePath,
      "--wal-dir", walDirectory,
      "--git-commit", git.commit,
    ], { required: false, severity: "error" });
    if (summaryStep.exit_code === 0) {
      try {
        const summary = parseJsonOutput(summaryStep);
        checks.push(...evaluateCaptureSummary(summary, hostProfile));
        if (importStep.exit_code === 0) {
          const verifyStep = await runStep(
            "capture-wal-verify",
            walBinary,
            ["verify", "--wal-dir", walDirectory],
            { required: false, severity: "error" },
          );
          if (verifyStep.exit_code === 0) {
            checks.push(...evaluateWalImport(
              parseJsonOutput(importStep),
              parseJsonOutput(verifyStep),
              summary.rows,
            ));
          }
        }
      } catch (error) {
        checks.push(check("summary.report_json", false, String(error.message), "valid JSON"));
      }
    }
    checks.push(check(
      "market_selection.formal",
      false,
      captureResult?.polymarket?.id ?? "dynamic",
      "versioned approved market ID",
      "warning",
    ));
  }
}

let runtime = null;
let git = null;
let fatalError = null;

try {
  checks.push(check("verification_profile.schema_version", profile.schema_version === 1, profile.schema_version, 1));
  checks.push(check("market_universe.schema_version", marketUniverse.schema_version === 1, marketUniverse.schema_version, 1));
  runtime = await runtimeChecks();
  git = await gitMetadata();
  await runStep("live-safety", process.execPath, ["scripts/check-live-safety.mjs"]);
  await runStep("readiness", process.execPath, ["scripts/report-readiness.mjs"], { required: false });
  const walBinary = await ensureWalBinary();
  if (mode === "local") {
    await runLocalVerification(walBinary, git);
  } else {
    await runHostSmoke(walBinary, git);
  }
} catch (error) {
  fatalError = String(error?.message ?? error);
  checks.push(check("pipeline.completed", false, fatalError, "completed"));
}

const status = overallStatus(checks);
const report = {
  schema_version: 1,
  run_id: runId,
  mode,
  status,
  started_at: startedAt,
  finished_at: new Date().toISOString(),
  output_directory: outputDirectory,
  verification_profile: {
    path: profilePath,
    schema_version: profile.schema_version,
    sha256: profileSha256,
  },
  configuration: {
    market_universe: {
      path: marketUniversePath,
      schema_version: marketUniverse.schema_version,
      sha256: marketUniverseSha256,
    },
  },
  runtime,
  git,
  parameters: {
    skip_checks: Boolean(args["skip-checks"]),
    no_build: Boolean(args["no-build"]),
    require_clean: Boolean(args["require-clean"]),
    duration_seconds: mode === "host-smoke"
      ? (args.duration === undefined ? profile.host_smoke.duration_seconds : Number(args.duration))
      : null,
    dns: mode === "host-smoke" ? String(args.dns ?? "doh") : null,
    symbol: mode === "host-smoke" ? String(args.symbol ?? "BTCUSDT").toUpperCase() : null,
    polymarket_query: mode === "host-smoke" ? String(args["polymarket-query"] ?? "bitcoin") : null,
  },
  safety_scope: {
    venue_writes: "disabled",
    credentials: "not accepted by this script",
    host_smoke_market_selection: "dynamic discovery is diagnostic only, never a formal benchmark universe",
  },
  fatal_error: fatalError,
  checks,
  steps: steps.map(({ stdout, ...step }) => step),
};
const reportPath = join(outputDirectory, "report.json");
await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, { flag: "wx" });
console.log(JSON.stringify({ status, mode, report: reportPath, checks: checks.length }, null, 2));
if (status === "failed") process.exitCode = 1;
