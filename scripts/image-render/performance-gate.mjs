import { readFileSync } from 'node:fs'
import { readFile, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const baseline = JSON.parse(
  readFileSync(
    new URL('../../docs/quality/image-renderer-baseline.json', import.meta.url),
    'utf8',
  ),
)

export const IMAGE_RENDER_BUDGETS = Object.freeze({
  ...baseline.budgets,
})

const REQUIRED_ROOT_FIELDS = Object.freeze([
  'schema_version',
  'measurement_mode',
  'frame_timing',
  'scenario',
  'magnifier_frame_samples',
  'workload',
  'frame_ms',
  'gpu_frame_ms',
  'gpu_timing',
  'gpu_timing_support',
  'gpu_sample_count',
  'presented_fps',
  'first_interactive_ms',
  'warm_first_interactive_ms',
  'gpu_texture_bytes_peak',
  'dropped_input_samples',
  'recovery_count',
  'save_commit_ms',
])

export function evaluateImageRenderReceipt(receipt, { expectedScenario } = {}) {
  const failures = []
  if (!isRecord(receipt)) return { passed: false, failures: ['receipt must be an object'] }
  for (const field of REQUIRED_ROOT_FIELDS) {
    if (!Object.hasOwn(receipt, field)) failures.push(`missing required field: ${field}`)
  }
  if (failures.length > 0) return { passed: false, failures }

  requireEqual(failures, 'schema_version', receipt.schema_version, 1)
  requireEqual(failures, 'measurement_mode', receipt.measurement_mode, 'native_metal_surface')
  requireEqual(failures, 'frame_timing', receipt.frame_timing, 'display_link_surface_submission')
  validateWorkload(receipt.workload, failures)
  if (!['main', 'magnifier'].includes(receipt.scenario)) {
    failures.push('scenario must be main or magnifier')
  }
  if (expectedScenario !== undefined) {
    requireEqual(failures, 'scenario', receipt.scenario, expectedScenario)
  }
  if (!Number.isInteger(receipt.magnifier_frame_samples) || receipt.magnifier_frame_samples < 0) {
    failures.push('magnifier_frame_samples must be a non-negative integer')
  }
  requireEqual(
    failures,
    'magnifier_frame_samples',
    receipt.magnifier_frame_samples,
    receipt.scenario === 'magnifier' ? receipt.workload?.frame_samples : 0,
  )
  validatePercentiles(receipt.frame_ms, 'frame_ms', failures)
  validatePercentiles(receipt.save_commit_ms, 'save_commit_ms', failures)
  if (receipt.gpu_timing === 'available') {
    validatePercentiles(receipt.gpu_frame_ms, 'gpu_frame_ms', failures)
  } else if (receipt.gpu_timing === 'unavailable') {
    if (receipt.gpu_frame_ms !== null) {
      failures.push('gpu_frame_ms must be null when gpu_timing is unavailable')
    }
  } else {
    failures.push('gpu_timing must be available or unavailable')
  }
  if (!Number.isInteger(receipt.gpu_sample_count) || receipt.gpu_sample_count < 0) {
    failures.push('gpu_sample_count must be a non-negative integer')
  }
  if (receipt.gpu_timing_support === 'available') {
    requireEqual(failures, 'gpu_timing', receipt.gpu_timing, 'available')
    requireEqual(failures, 'gpu_sample_count', receipt.gpu_sample_count, receipt.workload?.frame_samples)
  } else if (receipt.gpu_timing_support === 'unavailable') {
    requireEqual(failures, 'gpu_timing', receipt.gpu_timing, 'unavailable')
    requireEqual(failures, 'gpu_sample_count', receipt.gpu_sample_count, 0)
  } else {
    failures.push('gpu_timing_support must be available or unavailable')
  }

  for (const field of [
    'presented_fps',
    'first_interactive_ms',
    'warm_first_interactive_ms',
    'gpu_texture_bytes_peak',
    'dropped_input_samples',
    'recovery_count',
  ]) {
    requireFiniteNonNegative(failures, field, receipt[field])
  }
  if (Number.isFinite(receipt.first_interactive_ms) && receipt.first_interactive_ms <= 0) {
    failures.push('first_interactive_ms must be measured and positive')
  }
  if (
    Number.isFinite(receipt.warm_first_interactive_ms) &&
    receipt.warm_first_interactive_ms <= 0
  ) {
    failures.push('warm_first_interactive_ms must be measured and positive')
  }

  const displayHz = receipt.workload?.display_hz
  if (finiteNonNegative(displayHz) && displayHz > 0 && isRecord(receipt.frame_ms)) {
    maximum(failures, 'presented_fps', receipt.presented_fps, displayHz * 1.01)
    const frameBudgetMs = IMAGE_RENDER_BUDGETS.frameP95Ms
    if (receipt.frame_ms.p95 > frameBudgetMs) {
      failures.push(`frame_ms.p95 ${receipt.frame_ms.p95} exceeds ${frameBudgetMs.toFixed(2)}`)
    }
  }
  maximum(
    failures,
    'first_interactive_ms',
    receipt.first_interactive_ms,
    IMAGE_RENDER_BUDGETS.coldFirstInteractiveMs,
  )
  maximum(
    failures,
    'warm_first_interactive_ms',
    receipt.warm_first_interactive_ms,
    IMAGE_RENDER_BUDGETS.warmFirstInteractiveMs,
  )
  maximum(
    failures,
    'gpu_texture_bytes_peak',
    receipt.gpu_texture_bytes_peak,
    IMAGE_RENDER_BUDGETS.gpuTextureBytesPeak,
  )
  maximum(
    failures,
    'dropped_input_samples',
    receipt.dropped_input_samples,
    IMAGE_RENDER_BUDGETS.droppedInputSamples,
  )
  maximum(
    failures,
    'save_commit_ms.p95',
    receipt.save_commit_ms?.p95,
    IMAGE_RENDER_BUDGETS.saveP95Ms,
  )
  maximum(
    failures,
    'save_commit_ms.p99',
    receipt.save_commit_ms?.p99,
    IMAGE_RENDER_BUDGETS.saveP99Ms,
  )
  if (
    finiteNonNegative(receipt.presented_fps) &&
    Math.round(receipt.presented_fps * 100) / 100 < IMAGE_RENDER_BUDGETS.minimumPresentedFps
  ) {
    failures.push(`presented_fps ${receipt.presented_fps} is below 60`)
  }

  return { passed: failures.length === 0, failures }
}

export function mergeSaveLatency(receipt, ndjson) {
  const summaries = ndjson
    .split('\n')
    .filter((line) => line.trim().length > 0)
    .map((line) => JSON.parse(line))
    .filter((summary) => summary.phase === 'authoring')
  if (summaries.length === 0) throw new Error('save latency output contains no authoring samples')
  const scenarios = new Set()
  for (const summary of summaries) {
    if (!['asset', 'region'].includes(summary.scenario) || scenarios.has(summary.scenario)) {
      throw new Error('save latency requires unique asset and region scenarios')
    }
    scenarios.add(summary.scenario)
    for (const field of ['samples', 'p50Us', 'p95Us', 'p99Us']) {
      if (!Number.isInteger(summary[field]) || summary[field] < 0) {
        throw new Error(`save latency ${field} is invalid`)
      }
    }
    if (summary.samples < baseline.workload.saveSamples) {
      throw new Error(`save latency ${summary.scenario} has insufficient samples`)
    }
    if (summary.p50Us > summary.p95Us || summary.p95Us > summary.p99Us) {
      throw new Error(`save latency ${summary.scenario} percentiles are not monotonic`)
    }
  }
  if (scenarios.size !== 2) throw new Error('save latency requires both asset and region scenarios')
  return {
    ...receipt,
    workload: {
      ...receipt.workload,
      save_samples: summaries.reduce((total, summary) => total + summary.samples, 0),
    },
    save_commit_ms: {
      p50: Math.max(...summaries.map((summary) => summary.p50Us / 1000)),
      p95: Math.max(...summaries.map((summary) => summary.p95Us / 1000)),
      p99: Math.max(...summaries.map((summary) => summary.p99Us / 1000)),
    },
    save_measurement: 'viewer_continuous_review_authoring',
  }
}

export async function runImageRenderGate({ receiptPath, saveLatencyPath, expectedScenario }) {
  const receipt = JSON.parse(await readFile(receiptPath, 'utf8'))
  const merged = mergeSaveLatency(receipt, await readFile(saveLatencyPath, 'utf8'))
  const result = evaluateImageRenderReceipt(merged, { expectedScenario })
  await writeFile(receiptPath, `${JSON.stringify({ ...merged, gate: result }, null, 2)}\n`)
  return result
}

function validateWorkload(workload, failures) {
  if (!isRecord(workload)) {
    failures.push('workload must be an object')
    return
  }
  for (const field of [
    'source_width',
    'source_height',
    'annotation_count',
    'display_hz',
    'frame_samples',
    'save_samples',
  ]) {
    requireFiniteNonNegative(failures, `workload.${field}`, workload[field])
  }
  minimum(failures, 'workload.source_width', workload.source_width, baseline.workload.sourceWidth)
  minimum(failures, 'workload.source_height', workload.source_height, baseline.workload.sourceHeight)
  minimum(
    failures,
    'workload.annotation_count',
    workload.annotation_count,
    baseline.workload.annotationCount,
  )
  minimum(failures, 'workload.display_hz', workload.display_hz, baseline.workload.displayHz)
  minimum(
    failures,
    'workload.frame_samples',
    workload.frame_samples,
    baseline.workload.frameSamples,
  )
  minimum(
    failures,
    'workload.save_samples',
    workload.save_samples,
    baseline.workload.saveSamples,
  )
}

function validatePercentiles(value, path, failures) {
  if (!isRecord(value)) {
    failures.push(`${path} must be an object`)
    return
  }
  for (const percentile of ['p50', 'p95', 'p99']) {
    requireFiniteNonNegative(failures, `${path}.${percentile}`, value[percentile])
  }
  if (
    finiteNonNegative(value.p50) &&
    finiteNonNegative(value.p95) &&
    finiteNonNegative(value.p99) &&
    (value.p50 > value.p95 || value.p95 > value.p99)
  ) {
    failures.push(`${path} percentiles are not monotonic`)
  }
}

function requireEqual(failures, path, actual, expected) {
  if (actual !== expected) failures.push(`${path} must equal ${JSON.stringify(expected)}`)
}

function requireFiniteNonNegative(failures, path, value) {
  if (!finiteNonNegative(value)) failures.push(`${path} must be a finite non-negative number`)
}

function finiteNonNegative(value) {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0
}

function minimum(failures, path, value, limit) {
  if (finiteNonNegative(value) && value < limit) failures.push(`${path} ${value} is below ${limit}`)
}

function maximum(failures, path, value, limit) {
  if (finiteNonNegative(value) && value > limit) failures.push(`${path} ${value} exceeds ${limit}`)
}

function isRecord(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

function parseArguments(arguments_) {
  const allowed = new Set(['--receipt', '--save-latency', '--scenario'])
  const values = new Map()
  for (let index = 0; index < arguments_.length; index += 2) {
    const flag = arguments_[index]
    const value = arguments_[index + 1]
    if (!allowed.has(flag) || values.has(flag) || !value || value.startsWith('--')) return null
    values.set(flag, value)
  }
  if (values.size !== allowed.size || !['main', 'magnifier'].includes(values.get('--scenario'))) return null
  return {
    receiptPath: values.get('--receipt'),
    saveLatencyPath: values.get('--save-latency'),
    expectedScenario: values.get('--scenario'),
  }
}

const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : ''
if (invokedPath === fileURLToPath(import.meta.url)) {
  const arguments_ = parseArguments(process.argv.slice(2))
  if (arguments_ === null) {
    process.stderr.write('usage: performance-gate.mjs --receipt PATH --save-latency PATH --scenario main|magnifier\n')
    process.exitCode = 2
  } else {
    try {
      const result = await runImageRenderGate(arguments_)
      if (result.passed) process.stdout.write('native image renderer performance gate passed\n')
      else {
        process.stderr.write(`${result.failures.join('\n')}\n`)
        process.exitCode = 1
      }
    } catch (error) {
      process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`)
      process.exitCode = 2
    }
  }
}
