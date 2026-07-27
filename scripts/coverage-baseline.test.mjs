import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'

import {
  CRITICAL_UI_FILES,
  compareCoverage,
  compareCoverageReport,
  normalizeVitestReport,
  normalizeVitestSummary,
  readCoverageSummary,
} from './coverage-baseline.mjs'

const summary = (lines, branches, functions, statements) => ({
  total: {
    lines: { pct: lines },
    branches: { pct: branches },
    functions: { pct: functions },
    statements: { pct: statements },
  },
})
const fileSummary = (value) => ({
  lines: { pct: value },
  branches: { pct: value },
  functions: { pct: value },
  statements: { pct: value },
})
const metrics = (value) => ({
  lines: value,
  branches: value,
  functions: value,
  statements: value,
})

test('normalizes Vitest total percentages', () => {
  assert.deepEqual(normalizeVitestSummary(summary(80, 70, 75, 81)), {
    lines: 80,
    branches: 70,
    functions: 75,
    statements: 81,
  })
})

test('reads and normalizes a Vitest summary file', () => {
  const directory = mkdtempSync(join(tmpdir(), 'viewer-ui-coverage-'))
  const path = join(directory, 'coverage-summary.json')
  try {
    writeFileSync(path, JSON.stringify(summary(82, 72, 77, 83)))
    assert.deepEqual(readCoverageSummary(path), {
      lines: 82,
      branches: 72,
      functions: 77,
      statements: 83,
    })
  } finally {
    rmSync(directory, { recursive: true })
  }
})

test('supports importing the coverage API without a script argument', () => {
  const moduleUrl = new URL('./coverage-baseline.mjs', import.meta.url).href
  assert.doesNotThrow(() => {
    execFileSync(
      process.execPath,
      ['--input-type=module', '--eval', `await import(${JSON.stringify(moduleUrl)})`],
      { stdio: 'pipe' },
    )
  })
})

test('reports only metrics that exceed the allowed regression', () => {
  assert.deepEqual(
    compareCoverage(
      { lines: 78, branches: 69.5, functions: 75, statements: 80.5 },
      { lines: 80, branches: 70, functions: 75, statements: 81 },
      1,
    ),
    ['lines dropped from 80 to 78'],
  )
})

test('allows a regression exactly at the tolerance boundary', () => {
  assert.deepEqual(
    compareCoverage(
      { lines: 79, branches: 69, functions: 74, statements: 80 },
      { lines: 80, branches: 70, functions: 75, statements: 81 },
      1,
    ),
    [],
  )
})

test('reports a path-prefixed critical-file regression', () => {
  const regressedPath = CRITICAL_UI_FILES[4]
  const baseline = {
    global: metrics(80),
    critical: Object.fromEntries(CRITICAL_UI_FILES.map((path) => [path, metrics(80)])),
  }
  const current = {
    global: metrics(80),
    critical: Object.fromEntries(
      CRITICAL_UI_FILES.map((path) => [
        path,
        path === regressedPath ? { ...metrics(80), branches: 78 } : metrics(80),
      ]),
    ),
  }

  assert.deepEqual(compareCoverageReport(current, baseline, 1), [
    `${regressedPath}: branches dropped from 80 to 78`,
  ])
})

test('rejects a Vitest report with a missing critical file', () => {
  const report = {
    ...summary(80, 70, 75, 81),
    ...Object.fromEntries(
      CRITICAL_UI_FILES.slice(0, -1).map((path) => [`/workspace/ui/${path}`, fileSummary(80)]),
    ),
  }

  assert.throws(
    () => normalizeVitestReport(report),
    new Error(`coverage report is missing critical file ${CRITICAL_UI_FILES.at(-1)}`),
  )
})
