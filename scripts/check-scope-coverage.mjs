#!/usr/bin/env node

import { readFileSync } from "node:fs";

const productSpec = readFileSync("docs/PRODUCT_SPEC.md", "utf8");
const matrix = readFileSync("docs/milestones/viewer-0.1-scope-matrix.md", "utf8");
const idPattern = /\bREQ-[A-Z0-9]+(?:-[A-Z0-9]+)*\b/g;

const specIds = [...productSpec.matchAll(idPattern)].map(([id]) => id);
const specDuplicates = duplicates(specIds);
if (specDuplicates.length > 0) {
  throw new Error(`duplicate requirement IDs in product spec: ${specDuplicates.join(", ")}`);
}

const matrixRows = matrix
  .split(/\r?\n/)
  .filter((line) => /^\|\s*REQ-[A-Z0-9-]+\s*\|/.test(line))
  .map((line) => line.split("|").slice(1, -1).map((cell) => cell.trim()));
const matrixIds = matrixRows.map(([id]) => id);
const expectedM3Ids = [
  "REQ-FLOW-BATCH-RENAME",
  "REQ-FLOW-COMPARE",
  "REQ-FLOW-DRAG-DROP",
  "REQ-FLOW-EXTERNAL-CHANGES",
  "REQ-FLOW-LIFECYCLE",
  "REQ-FLOW-ORGANIZE",
  "REQ-FLOW-READONLY-ERRORS",
  "REQ-FLOW-SHORTCUTS",
  "REQ-FLOW-UNDO",
  "REQ-IA-TASK-BAR",
  "REQ-TECH-FILE-CONSISTENCY",
  "REQ-TECH-PATH-SECURITY",
  "REQ-TECH-WATCHER",
];
const matrixDuplicates = duplicates(matrixIds);
if (matrixDuplicates.length > 0) {
  throw new Error(`duplicate requirement IDs in scope matrix: ${matrixDuplicates.join(", ")}`);
}

const specSet = new Set(specIds);
const matrixSet = new Set(matrixIds);
const missing = specIds.filter((id) => !matrixSet.has(id));
const unknown = matrixIds.filter((id) => !specSet.has(id));
if (missing.length > 0 || unknown.length > 0) {
  throw new Error(
    `scope coverage mismatch; missing=[${missing.join(", ")}], unknown=[${unknown.join(", ")}]`,
  );
}

for (const row of matrixRows) {
  if (row.length !== 7) {
    throw new Error(`matrix row ${row[0]} must contain exactly 7 columns`);
  }
  const [id, summary, owner, milestone, tests, acceptance, gate] = row;
  for (const [name, value] of Object.entries({ summary, owner, milestone, tests, acceptance, gate })) {
    if (!value || value === "—") {
      throw new Error(`matrix row ${id} has an empty ${name} column`);
    }
  }
  if (!/^M[1-4](?:\/M[1-4])*$/.test(milestone)) {
    throw new Error(`matrix row ${id} has invalid milestone ${milestone}`);
  }
}

const actualM3Ids = matrixRows
  .filter(([, , , milestone]) => milestone.split("/").includes("M3"))
  .map(([id]) => id)
  .sort();
if (JSON.stringify(actualM3Ids) !== JSON.stringify(expectedM3Ids)) {
  throw new Error(
    `M3 scope changed without review; expected=[${expectedM3Ids.join(", ")}], actual=[${actualM3Ids.join(", ")}]`,
  );
}

if (specIds.length === 0) {
  throw new Error("product spec contains no stable requirement IDs");
}

console.log(
  `Viewer scope coverage passed: ${specIds.length} requirements mapped exactly once; ${actualM3Ids.length} frozen M3 requirements`,
);

function duplicates(values) {
  const seen = new Set();
  const repeated = new Set();
  for (const value of values) {
    if (seen.has(value)) repeated.add(value);
    seen.add(value);
  }
  return [...repeated].sort();
}
