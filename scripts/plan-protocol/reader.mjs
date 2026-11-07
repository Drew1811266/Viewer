import { spawn } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const protocolVersion = 'viewer.plan.reader/1'

export function readerExecutable(environment = process.env) {
  return environment.VIEWER_PLAN_READER ?? fileURLToPath(new URL('../../target/debug/viewer-plan-reader', import.meta.url))
}

export function invokePlanReader(operation, options = {}) {
  const request = JSON.stringify({ ...options, protocolVersion, operation })
  return new Promise((resolve, reject) => {
    const child = spawn(readerExecutable(), [], { stdio: ['pipe', 'pipe', 'pipe'], shell: false })
    const chunks = []
    let stderr = ''
    child.on('error', reject)
    child.stdout.on('data', (chunk) => chunks.push(chunk))
    child.stderr.on('data', (chunk) => { stderr += chunk })
    child.on('close', (code) => {
      const text = Buffer.concat(chunks).toString('utf8').trim()
      if (!text) {
        reject(Object.assign(new Error(stderr || 'plan reader produced no output'), { code: 'io' }))
        return
      }
      const result = JSON.parse(text)
      if (code === 0 && result.status === 'ok') resolve(result)
      else reject(Object.assign(new Error(result.message ?? 'plan reader failed'), { code: result.code ?? 'io', result }))
    })
    child.stdin.end(request)
  })
}
