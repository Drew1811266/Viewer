import { spawn } from 'node:child_process'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const protocolVersion = 'viewer.review.reader/1'
const maxRequestBytes = 64 * 1024
const maxResponseBytes = 64 * 1024 * 1024
const codes = new Set(['migration_required', 'unsupported_version', 'integrity', 'unsafe_path',
  'limit_exceeded', 'ambiguous_stream', 'unknown_stream', 'unknown_history', 'io'])

function failure(code, message) { return Object.assign(new Error(message), { code }) }

export function readerExecutable(environment = process.env) {
  const override = environment.VIEWER_REVIEW_READER
  if (override !== undefined) {
    if (!path.isAbsolute(override) || override.includes('\0')) {
      throw failure('io', 'VIEWER_REVIEW_READER must be an explicit absolute executable path')
    }
    return override
  }
  return fileURLToPath(new URL('../../target/debug/viewer-review-reader', import.meta.url))
}

export async function invokeReader(operation, options = {}, { signal } = {}) {
  const executable = readerExecutable()
  // Caller options cannot replace the transport protocol or operation.
  const request = JSON.stringify({ ...options, protocolVersion, operation })
  if (Buffer.byteLength(request) > maxRequestBytes) throw failure('limit_exceeded', 'reader request exceeds 64 KiB')
  return new Promise((resolve, reject) => {
    const child = spawn(executable, [], { stdio: ['pipe', 'pipe', 'pipe'], shell: false, signal })
    const chunks = []
    let bytes = 0
    let stderrBytes = 0
    let settled = false
    const fail = (error) => {
      if (settled) return
      settled = true
      child.kill()
      reject(error)
    }
    child.on('error', error => fail(failure('io', error.name === 'AbortError'
      ? 'review read was cancelled'
      : 'Cannot run the read-only core; run pnpm build:review-reader or set VIEWER_REVIEW_READER')))
    child.stdin.on('error', () => {}) // Exit/close below owns the single transport outcome.
    child.stdout.on('data', (chunk) => {
      bytes += chunk.length
      if (bytes > maxResponseBytes) fail(failure('limit_exceeded', 'reader response exceeds 64 MiB'))
      else chunks.push(chunk)
    })
    child.stderr.on('data', (chunk) => {
      stderrBytes += chunk.length
      if (stderrBytes > 8192) fail(failure('io', 'reader returned excessive diagnostics'))
    })
    child.on('close', (exitCode) => {
      if (settled) return
      try {
        const response = JSON.parse(Buffer.concat(chunks, bytes).toString('utf8'))
        if (response.protocolVersion !== protocolVersion || stderrBytes !== 0) throw failure('io', 'invalid reader transport response')
        if (exitCode === 0 && Object.keys(response).sort().join(',') === 'protocolVersion,result') {
          settled = true
          resolve(response.result)
          return
        }
        if (exitCode === 1 && Object.keys(response).sort().join(',') === 'error,protocolVersion'
            && codes.has(response.error?.code) && typeof response.error.message === 'string'
            && response.error.message.length > 0 && Buffer.byteLength(response.error.message) <= 4096) {
          throw failure(response.error.code, response.error.message)
        }
        throw failure('io', 'reader exited without a valid result')
      } catch (error) { fail(codes.has(error.code) ? error : failure('io', 'reader returned invalid JSON')) }
    })
    child.stdin.end(request)
  })
}
