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

if (specIds.length === 0) {
  throw new Error("product spec contains no stable requirement IDs");
}

console.log(`Viewer scope coverage passed: ${specIds.length} requirements mapped exactly once`);

function duplicates(values) {
  const seen = new Set();
  const repeated = new Set();
  for (const value of values) {
    if (seen.has(value)) repeated.add(value);
    seen.add(value);
  }
  return [...repeated].sort();
}
