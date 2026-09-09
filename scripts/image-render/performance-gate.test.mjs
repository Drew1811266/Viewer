import assert from 'node:assert/strict'
import { execFileSync, spawnSync } from 'node:child_process'
import { mkdtempSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, it } from 'node:test'

import { evaluateImageRenderReceipt, mergeSaveLatency } from './performance-gate.mjs'

function passingReceipt() {
  return {
    schema_version: 1,
    measurement_mode: 'native_metal_surface',
    frame_timing: 'display_link_surface_submission',
    scenario: 'main',
    magnifier_frame_samples: 0,
    workload: {
      source_width: 7680,
      source_height: 4320,
      annotation_count: 500,
      display_hz: 60,
      frame_samples: 240,
      save_samples: 30,
    },
    frame_ms: { p50: 8.1, p95: 15.9, p99: 16.6 },
    gpu_frame_ms: null,
    gpu_timing: 'unavailable',
    gpu_timing_support: 'unavailable',
    gpu_sample_count: 0,
    presented_fps: 60.1,
    first_interactive_ms: 250,
    warm_first_interactive_ms: 80,
    gpu_texture_bytes_peak: 150 * 1024 * 1024,
    dropped_input_samples: 0,
    recovery_count: 1,
    save_commit_ms: { p50: 12, p95: 80, p99: 220 },
  }
}

describe('image renderer performance gate', () => {
  it('accepts a complete receipt at the hard budgets', () => {
    const result = evaluateImageRenderReceipt(passingReceipt())
    assert.equal(result.passed, true)
    assert.deepEqual(result.failures, [])
  })

  it('accepts complete asynchronously collected GPU samples when the adapter supports timing', () => {
    const receipt = passingReceipt()
    receipt.gpu_timing_support = 'available'
    receipt.gpu_timing = 'available'
    receipt.gpu_sample_count = 240
    receipt.gpu_frame_ms = { p50: 1, p95: 2, p99: 3 }
    assert.equal(evaluateImageRenderReceipt(receipt).passed, true)
  })

  it('requires every magnifier workload frame to actually encode the lens pass', () => {
    const receipt = { ...passingReceipt(), scenario: 'magnifier', magnifier_frame_samples: 240 }
    assert.equal(evaluateImageRenderReceipt(receipt).passed, true)
    for (const samples of [0, 239, 241, 0.5, '240', Number.NaN]) {
      assert.equal(evaluateImageRenderReceipt({ ...receipt, magnifier_frame_samples: samples }).passed, false)
    }
  })

  it('rejects mislabeled baseline or substituted magnifier receipts', () => {
    assert.equal(evaluateImageRenderReceipt({ ...passingReceipt(), magnifier_frame_samples: 1 }).passed, false)
    assert.equal(evaluateImageRenderReceipt({ ...passingReceipt(), scenario: 'unknown' }).passed, false)
    assert.equal(evaluateImageRenderReceipt(passingReceipt(), { expectedScenario: 'magnifier' }).passed, false)
  })

  it('rejects missing GPU samples instead of reporting supported-but-unmeasured as unavailable', () => {
    const receipt = passingReceipt()
    receipt.gpu_timing_support = 'available'
    assert.equal(evaluateImageRenderReceipt(receipt).passed, false)
    receipt.gpu_timing = 'available'
    receipt.gpu_frame_ms = { p50: 1, p95: 2, p99: 3 }
    receipt.gpu_sample_count = 239
    assert.equal(evaluateImageRenderReceipt(receipt).passed, false)
  })

  it('rejects contradictory or malformed GPU capability evidence', () => {
    for (const count of [1, -1, 0.5, Number.NaN, '0']) {
      const receipt = passingReceipt()
      receipt.gpu_sample_count = count
      assert.equal(evaluateImageRenderReceipt(receipt).passed, false)
    }
    const receipt = passingReceipt()
    receipt.gpu_timing_support = 'unknown'
    assert.equal(evaluateImageRenderReceipt(receipt).passed, false)
  })

  it('rejects the old offscreen submission receipt as native window evidence', () => {
    const receipt = passingReceipt()
    receipt.measurement_mode = 'native_metal'
    receipt.presented_fps = 5562
    assert.equal(evaluateImageRenderReceipt(receipt).passed, false)
  })

  it('rejects a reported surface rate that exceeds the physical display cadence', () => {
    const receipt = passingReceipt()
    receipt.presented_fps = 5000
    assert.equal(evaluateImageRenderReceipt(receipt).passed, false)
  })

  it('uses hundredth-FPS precision for nominal 60 Hz host-clock rounding', () => {
    const receipt = passingReceipt()
    receipt.presented_fps = 59.99990019932918
    assert.equal(evaluateImageRenderReceipt(receipt).passed, true)
  })

  for (const [name, mutate] of [
    ['60 Hz frame p95', (value) => (value.frame_ms.p95 = 16.71)],
    ['continuous presentation rate', (value) => (value.presented_fps = 59.99)],
    ['cold first interactive', (value) => (value.first_interactive_ms = 300.01)],
    ['warm first interactive', (value) => (value.warm_first_interactive_ms = 100.01)],
    ['GPU texture peak', (value) => (value.gpu_texture_bytes_peak = 256 * 1024 * 1024 + 1)],
    ['lossless input boundaries', (value) => (value.dropped_input_samples = 1)],
    ['save p95', (value) => (value.save_commit_ms.p95 = 100.01)],
    ['save p99', (value) => (value.save_commit_ms.p99 = 300.01)],
    ['8K width', (value) => (value.workload.source_width = 4096)],
    ['8K height', (value) => (value.workload.source_height = 2160)],
    ['500 annotations', (value) => (value.workload.annotation_count = 499)],
    ['240 frame samples', (value) => (value.workload.frame_samples = 239)],
    ['30 save samples', (value) => (value.workload.save_samples = 29)],
  ]) {
    it(`rejects an over-budget or incomplete ${name} measurement`, () => {
      const receipt = passingReceipt()
      mutate(receipt)
      const result = evaluateImageRenderReceipt(receipt)
      assert.equal(result.passed, false)
      assert.ok(result.failures.length > 0)
    })
  }

  it('rejects every missing required field instead of treating it as zero', () => {
    for (const field of Object.keys(passingReceipt())) {
      const receipt = passingReceipt()
      delete receipt[field]
      assert.equal(evaluateImageRenderReceipt(receipt).passed, false, field)
    }
  })

  it('requires a finite non-negative value for every numeric measurement', () => {
    for (const invalid of [Number.NaN, Number.POSITIVE_INFINITY, -1, '12']) {
      const receipt = passingReceipt()
      receipt.frame_ms.p50 = invalid
      assert.equal(evaluateImageRenderReceipt(receipt).passed, false)
    }
  })

  it('merges only real foreground authoring latency and preserves the slowest scenario', () => {
    const merged = mergeSaveLatency(
      passingReceipt(),
      [
        JSON.stringify({
          phase: 'authoring',
          scenario: 'asset',
          samples: 30,
          p50Us: 10_000,
          p95Us: 80_000,
          p99Us: 220_000,
        }),
        JSON.stringify({
          phase: 'materialization',
          scenario: 'asset',
          samples: 30,
          p50Us: 500_000,
          p95Us: 600_000,
          p99Us: 700_000,
        }),
        JSON.stringify({
          phase: 'authoring',
          scenario: 'region',
          samples: 30,
          p50Us: 12_000,
          p95Us: 90_000,
          p99Us: 250_000,
        }),
      ].join('\n'),
    )
    assert.equal(merged.workload.save_samples, 60)
    assert.deepEqual(merged.save_commit_ms, { p50: 12, p95: 90, p99: 250 })
    assert.equal(merged.save_measurement, 'viewer_continuous_review_authoring')
  })

  it('requires both independent authoring scenarios instead of inflating one into total coverage', () => {
    const asset = { phase: 'authoring', scenario: 'asset', samples: 30, p50Us: 10, p95Us: 20, p99Us: 30 }
    for (const summaries of [
      [asset],
      [asset, asset],
      [asset, { ...asset, scenario: 'region', samples: 0 }],
      [asset, { ...asset, scenario: 'unknown' }],
      [asset, { ...asset, scenario: 'region', p50Us: 40 }],
    ]) {
      assert.throws(() => mergeSaveLatency(passingReceipt(), summaries.map(JSON.stringify).join('\n')))
    }
  })

  it('fails the real CLI when a main receipt is substituted for the required lens run', (t) => {
    const temporary = mkdtempSync(path.join(os.tmpdir(), 'viewer-performance-scenario-'))
    t.after(() => rmSync(temporary, { recursive: true, force: true }))
    const receiptPath = path.join(temporary, 'receipt.json')
    const savePath = path.join(temporary, 'save.ndjson')
    writeFileSync(receiptPath, JSON.stringify(passingReceipt()))
    writeFileSync(savePath, ['asset', 'region'].map((scenario) => JSON.stringify({
      phase: 'authoring', scenario, samples: 30, p50Us: 100, p95Us: 200, p99Us: 300,
    })).join('\n'))
    const command = [fileURLToPath(new URL('./performance-gate.mjs', import.meta.url)),
      '--receipt', receiptPath, '--save-latency', savePath, '--scenario']
    const mismatch = spawnSync(process.execPath, [...command, 'magnifier'], { encoding: 'utf8' })
    assert.equal(mismatch.status, 1)
    assert.match(mismatch.stderr, /scenario must equal "magnifier"/)
    assert.equal(JSON.parse(readFileSync(receiptPath, 'utf8')).gate.passed, false)
    const valid = spawnSync(process.execPath, [...command, 'main'], { encoding: 'utf8' })
    assert.equal(valid.status, 0, valid.stderr)
    assert.equal(JSON.parse(readFileSync(receiptPath, 'utf8')).gate.passed, true)
    const invalid = spawnSync(process.execPath, [...command, 'unexpected'], { encoding: 'utf8' })
    assert.equal(invalid.status, 2)
    const saved = readFileSync(receiptPath, 'utf8')
    for (const extra of [
      ['--scenario', 'main'],
      ['--scenario', 'magnifier'],
      ['--receipt', receiptPath],
      ['--save-latency', savePath],
      ['unexpected'],
      ['--unexpected', 'value'],
      ['--scenario'],
    ]) {
      const malformed = spawnSync(process.execPath, [...command, 'main', ...extra], { encoding: 'utf8' })
      assert.equal(malformed.status, 2, JSON.stringify(extra))
      assert.match(malformed.stderr, /usage:/)
      assert.equal(readFileSync(receiptPath, 'utf8'), saved)
    }
  })

  it('keeps the deterministic 8K workload separate from an optional user corpus', (t) => {
    const temporary = mkdtempSync(path.join(os.tmpdir(), 'viewer-performance-runner-'))
    t.after(() => rmSync(temporary, { recursive: true, force: true }))
    const bin = path.join(temporary, 'bin')
    const corpus = path.join(temporary, '测试图 with spaces')
    const output = path.join(temporary, 'owned-output')
    const log = path.join(temporary, 'calls.ndjson')
    mkdirSync(bin)
    mkdirSync(corpus)
    writeFileSync(path.join(corpus, 'original.png'), 'untouched original')
    // These test doubles exercise shell argument/ownership routing, not GPU
    // performance. The real Metal measurement remains a separate gate.
    for (const tool of ['cargo', 'swift', 'node']) {
      writeFileSync(path.join(bin, tool), `#!${process.execPath}
const fs = require('node:fs');
const args = process.argv.slice(2);
fs.appendFileSync(process.env.VIEWER_RUNNER_CALLS, JSON.stringify({tool: '${tool}', args}) + '\\n');
if ('${tool}' === 'swift') fs.writeFileSync(args[args.indexOf('--output') + 1], 'generated fixture');
if ('${tool}' === 'cargo' && args.includes('image_render_acceptance')) {
  if (!fs.existsSync(args[args.indexOf('--') + 1])) process.exit(2);
}
`, { mode: 0o755 })
    }
    execFileSync('/bin/sh', [fileURLToPath(new URL('./run-performance-gate.sh', import.meta.url))], {
      env: {
        ...process.env,
        PATH: `${bin}:${process.env.PATH}`,
        VIEWER_RUNNER_CALLS: log,
        VIEWER_IMAGE_RENDER_OUTPUT_DIR: output,
        VIEWER_IMAGE_RENDER_FIXTURE_ROOT: corpus,
      },
      stdio: 'pipe',
    })
    const calls = readFileSync(log, 'utf8').trim().split('\n').map(JSON.parse)
    const render = calls.find((call) => call.args.includes('image_render_acceptance'))
    const save = calls.find((call) => call.args.includes('review_save_latency'))
    assert.equal(render.args[render.args.indexOf('--') + 1], path.join(output, 'fixture', 'viewer-native-8k.jpg'))
    const renders = calls.filter((call) => call.args.includes('image_render_acceptance'))
    assert.equal(renders.length, 2)
    assert.deepEqual(renders.map((call) => call.args.at(-1)), ['main', 'magnifier'])
    const gates = calls.filter((call) => call.tool === 'node')
    assert.equal(gates.length, 2)
    assert.deepEqual(gates.map((call) => call.args[call.args.indexOf('--scenario') + 1]), ['main', 'magnifier'])
    assert.notEqual(renders[0].args.at(-2), renders[1].args.at(-2))
    assert.equal(save.args[save.args.indexOf('--project') + 1], corpus)
    assert.deepEqual(readdirSync(corpus), ['original.png'])
    assert.equal(readFileSync(path.join(corpus, 'original.png'), 'utf8'), 'untouched original')
  })
})
