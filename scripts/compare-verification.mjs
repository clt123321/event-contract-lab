#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

import { compareReports } from "./lib/verification.mjs";

function parseArgs(argv) {
  const result = {};
  for (let index = 0; index < argv.length; index += 2) {
    const flag = argv[index];
    const value = argv[index + 1];
    if (!new Set(["--before", "--after"]).has(flag) || value === undefined) {
      throw new Error("Usage: compare-verification.mjs --before <report.json> --after <report.json>");
    }
    result[flag.slice(2)] = value;
  }
  if (!result.before || !result.after) {
    throw new Error("Both --before and --after are required");
  }
  return result;
}

const args = parseArgs(process.argv.slice(2));
const [before, after] = await Promise.all([
  readFile(resolve(args.before), "utf8").then(JSON.parse),
  readFile(resolve(args.after), "utf8").then(JSON.parse),
]);
if (before.schema_version !== 1 || after.schema_version !== 1) {
  throw new Error("Only verification report schema version 1 is supported");
}
const { transitions: _transitions, ...summary } = compareReports(before, after);
console.log(JSON.stringify(summary, null, 2));
