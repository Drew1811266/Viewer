#!/usr/bin/env node

import { readFileSync, writeFileSync } from 'node:fs'
import { pathToFileURL } from 'node:url'

const METRICS = ['lines', 'functions', 'regions']
export const CRITICAL_RUST_GROUPS = {
  'infrastructure-operation': ['crates/viewer-infrastructure/src/operation/'],
  'desktop-security-boundaries': [
    'src-tauri/src/commands/',
    'src-tauri/src/error.rs',
    'src-tauri/src/image_protocol.rs',
    'src-tauri/src/markdown.rs',
  ],
  'application-project-lifecycle': [
    'crates/viewer-application/src/project.rs',
    'crates/viewer-application/src/session.rs',
    'crates/viewer-application/src/operation_commit.rs',
  ],
}

export function normalizeLlvmCoverage(report) {
  const totals = report.data[0].totals
  return {
    lines: totals.lines.percent,
    functions: totals.functions.percent,
    regions: totals.regions.percent,
  }
}

function aggregateFiles(files) {
  return Object.fromEntries(METRICS.map((metric) => {
    const totals = files.reduce(
      (sum, file) => ({
        count: sum.count + file.summary[metric].count,
        covered: sum.covered + file.summary[metric].covered,
      }),
      { count: 0, covered: 0 },
    )
    if (totals.count === 0) throw new Error(`critical Rust group has no ${metric} data`)
    return [metric, (totals.covered / totals.count) * 100]
  }))
}

export function normalizeLlvmReport(report) {
  const files = report.data[0].files
  const critical = Object.fromEntries(
    Object.entries(CRITICAL_RUST_GROUPS).map(([group, prefixes]) => {
      const selected = files.filter((file) => {
        const path = file.filename.replaceAll('\\', '/')
        return prefixes.some((prefix) => path.endsWith(prefix) || path.includes(`/${prefix}`))
      })
      if (selected.length === 0) throw new Error(`coverage report is missing ${group}`)
      return [group, aggregateFiles(selected)]
    }),
  )
  return { global: normalizeLlvmCoverage(report), critical }
}

export function compareRustCoverage(current, baseline, tolerance = 1) {
  return METRICS.flatMap((metric) =>
    current[metric] + tolerance < baseline[metric]
      ? [`${metric} dropped from ${baseline[metric]} to ${current[metric]}`]
      : [],
  )
}

export function compareRustCoverageReport(current, baseline, tolerance = 1) {
  const failures = compareRustCoverage(current.global, baseline.global, tolerance)
  for (const group of Object.keys(CRITICAL_RUST_GROUPS)) {
    failures.push(
      ...compareRustCoverage(current.critical[group], baseline.critical[group], tolerance)
        .map((failure) => `${group}: ${failure}`),
    )
  }
  return failures
}

export function main(argv = process.argv.slice(2)) {
  const [mode, reportPath, baselinePath] = argv
  if (!reportPath || !baselinePath) {
    throw new Error('usage: rust-coverage-baseline.mjs --update|--check report baseline')
  }
  const current = normalizeLlvmReport(JSON.parse(readFileSync(reportPath, 'utf8')))
  if (mode === '--update') {
    writeFileSync(baselinePath, `${JSON.stringify(current, null, 2)}\n`)
    return 0
  }
  if (mode !== '--check') {
    throw new Error('usage: rust-coverage-baseline.mjs --update|--check report baseline')
  }
  const failures = compareRustCoverageReport(
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
