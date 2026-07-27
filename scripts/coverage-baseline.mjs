#!/usr/bin/env node

import { readFileSync, writeFileSync } from 'node:fs'
import { pathToFileURL } from 'node:url'

const METRICS = ['lines', 'branches', 'functions', 'statements']
export const CRITICAL_UI_FILES = [
  'src/state/reducers/projectReducer.ts',
  'src/state/reducers/workspaceReducer.ts',
  'src/state/reducers/searchReducer.ts',
  'src/state/reducers/operationReducer.ts',
  'src/state/controllers/useProjectSessionController.ts',
  'src/state/controllers/useOperationController.ts',
]

export function normalizeVitestSummary(summary) {
  return Object.fromEntries(METRICS.map((metric) => [metric, summary.total[metric].pct]))
}

export function readCoverageSummary(path) {
  return normalizeVitestSummary(JSON.parse(readFileSync(path, 'utf8')))
}

export function normalizeVitestReport(summary) {
  const critical = Object.fromEntries(CRITICAL_UI_FILES.map((path) => {
    const entry = Object.entries(summary).find(
      ([candidate]) => candidate === path || candidate.endsWith(`/${path}`),
    )
    if (!entry) throw new Error(`coverage report is missing critical file ${path}`)
    return [
      path,
      Object.fromEntries(METRICS.map((metric) => [metric, entry[1][metric].pct])),
    ]
  }))
  return { global: normalizeVitestSummary(summary), critical }
}

export function compareCoverage(current, baseline, tolerance = 1) {
  return METRICS.flatMap((metric) =>
    current[metric] + tolerance < baseline[metric]
      ? [`${metric} dropped from ${baseline[metric]} to ${current[metric]}`]
      : [],
  )
}

export function compareCoverageReport(current, baseline, tolerance = 1) {
  const failures = compareCoverage(current.global, baseline.global, tolerance)
  for (const path of CRITICAL_UI_FILES) {
    failures.push(
      ...compareCoverage(current.critical[path], baseline.critical[path], tolerance)
        .map((failure) => `${path}: ${failure}`),
    )
  }
  return failures
}

export function main(argv = process.argv.slice(2)) {
  const [mode, summaryPath, baselinePath] = argv
  const current = normalizeVitestReport(JSON.parse(readFileSync(summaryPath, 'utf8')))
  if (mode === '--update') {
    writeFileSync(baselinePath, `${JSON.stringify(current, null, 2)}\n`)
    return 0
  }
  if (mode !== '--check') throw new Error('usage: coverage-baseline.mjs --update|--check summary baseline')
  const failures = compareCoverageReport(
    current,
    JSON.parse(readFileSync(baselinePath, 'utf8')),
  )
  if (failures.length > 0) {
    process.stderr.write(`${failures.join('\n')}\n`)
    return 1
  }
  return 0
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.exitCode = main()
}
