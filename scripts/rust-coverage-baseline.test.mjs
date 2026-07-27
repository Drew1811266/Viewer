import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import test from 'node:test'

import {
  compareRustCoverage,
  compareRustCoverageReport,
  normalizeLlvmReport,
  normalizeLlvmCoverage,
} from './rust-coverage-baseline.mjs'

test('normalizes cargo llvm-cov totals', () => {
  const report = {
    data: [{
      totals: {
        lines: { percent: 81.25 },
        functions: { percent: 76.5 },
        regions: { percent: 79.75 },
      },
    }],
  }
  assert.deepEqual(normalizeLlvmCoverage(report), {
    lines: 81.25,
    functions: 76.5,
    regions: 79.75,
  })
})

test('supports importing the Rust coverage API without a script argument', () => {
  const moduleUrl = new URL('./rust-coverage-baseline.mjs', import.meta.url).href
  assert.doesNotThrow(() => {
    execFileSync(
      process.execPath,
      ['--input-type=module', '--eval', `await import(${JSON.stringify(moduleUrl)})`],
      { stdio: 'pipe' },
    )
  })
})

test('reports Rust metrics that regress beyond one percentage point', () => {
  assert.deepEqual(
    compareRustCoverage(
      { lines: 79, functions: 75.5, regions: 74 },
      { lines: 81, functions: 76, regions: 75 },
      1,
    ),
    ['lines dropped from 81 to 79'],
  )
})

test('prefixes a critical Rust group regression with its group name', () => {
  const stable = { lines: 80, functions: 70, regions: 75 }
  const baseline = {
    global: { ...stable },
    critical: {
      'infrastructure-operation': { ...stable },
      'desktop-security-boundaries': { ...stable },
      'application-project-lifecycle': { ...stable },
    },
  }
  const current = {
    global: { ...stable },
    critical: {
      'infrastructure-operation': { ...stable },
      'desktop-security-boundaries': { ...stable, functions: 68 },
      'application-project-lifecycle': { ...stable },
    },
  }

  assert.deepEqual(
    compareRustCoverageReport(current, baseline, 1),
    ['desktop-security-boundaries: functions dropped from 70 to 68'],
  )
})

test('aggregates all critical Rust groups from relative path matches', () => {
  const report = {
    data: [{
      totals: {
        lines: { percent: 70 },
        functions: { percent: 60 },
        regions: { percent: 50 },
      },
      files: [
        {
          filename: '/workspace/crates/viewer-infrastructure/src/operation/a.rs',
          summary: {
            lines: { count: 10, covered: 8 },
            functions: { count: 5, covered: 4 },
            regions: { count: 20, covered: 15 },
          },
        },
        {
          filename: 'C:\\workspace\\crates\\viewer-infrastructure\\src\\operation\\b.rs',
          summary: {
            lines: { count: 30, covered: 12 },
            functions: { count: 5, covered: 1 },
            regions: { count: 20, covered: 5 },
          },
        },
        {
          filename: '/workspace/src-tauri/src/commands/project.rs',
          summary: {
            lines: { count: 10, covered: 9 },
            functions: { count: 4, covered: 3 },
            regions: { count: 12, covered: 9 },
          },
        },
        {
          filename: '/workspace/crates/viewer-application/src/project.rs',
          summary: {
            lines: { count: 20, covered: 10 },
            functions: { count: 8, covered: 2 },
            regions: { count: 16, covered: 4 },
          },
        },
      ],
    }],
  }

  assert.deepEqual(normalizeLlvmReport(report), {
    global: { lines: 70, functions: 60, regions: 50 },
    critical: {
      'infrastructure-operation': { lines: 50, functions: 50, regions: 50 },
      'desktop-security-boundaries': { lines: 90, functions: 75, regions: 75 },
      'application-project-lifecycle': { lines: 50, functions: 25, regions: 25 },
    },
  })
})

test('rejects reports missing a critical Rust group', () => {
  const report = {
    data: [{
      totals: {
        lines: { percent: 0 },
        functions: { percent: 0 },
        regions: { percent: 0 },
      },
      files: [
        {
          filename: '/workspace/crates/viewer-infrastructure/src/operation/a.rs',
          summary: {
            lines: { count: 1, covered: 0 },
            functions: { count: 1, covered: 0 },
            regions: { count: 1, covered: 0 },
          },
        },
      ],
    }],
  }

  assert.throws(
    () => normalizeLlvmReport(report),
    /coverage report is missing desktop-security-boundaries/,
  )
})

test('rejects critical Rust groups without metric data', () => {
  const report = {
    data: [{
      totals: {
        lines: { percent: 0 },
        functions: { percent: 0 },
        regions: { percent: 0 },
      },
      files: [
        {
          filename: '/workspace/crates/viewer-infrastructure/src/operation/a.rs',
          summary: {
            lines: { count: 0, covered: 0 },
            functions: { count: 1, covered: 0 },
            regions: { count: 1, covered: 0 },
          },
        },
        {
          filename: '/workspace/src-tauri/src/error.rs',
          summary: {
            lines: { count: 1, covered: 0 },
            functions: { count: 1, covered: 0 },
            regions: { count: 1, covered: 0 },
          },
        },
        {
          filename: '/workspace/crates/viewer-application/src/project.rs',
          summary: {
            lines: { count: 1, covered: 0 },
            functions: { count: 1, covered: 0 },
            regions: { count: 1, covered: 0 },
          },
        },
      ],
    }],
  }

  assert.throws(
    () => normalizeLlvmReport(report),
    /critical Rust group has no lines data/,
  )
})
