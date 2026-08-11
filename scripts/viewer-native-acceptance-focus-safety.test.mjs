import assert from 'node:assert/strict'
import { execFile, spawn } from 'node:child_process'
import { mkdtemp, readFile, rm } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { after, before, test } from 'node:test'
import { promisify } from 'node:util'

const execFileAsync = promisify(execFile)
const sourcePath = new URL('./viewer-native-acceptance.swift', import.meta.url).pathname
let temporaryRoot
let helperPath

before(async () => {
  temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-focus-safety-'))
  helperPath = path.join(temporaryRoot, 'viewer-native-acceptance-helper')
  await execFileAsync('xcrun', ['swiftc', '-warnings-as-errors', sourcePath, '-o', helperPath])
})

after(async () => {
  await rm(temporaryRoot, { recursive: true, force: true })
})

async function runJsonMode(args) {
  const { stdout } = await execFileAsync(helperPath, args, { encoding: 'utf8', timeout: 3_000 })
  return JSON.parse(stdout.trim())
}

async function runRequest(args, request) {
  const child = spawn(helperPath, args, { stdio: ['pipe', 'pipe', 'pipe'] })
  const stdout = []
  const stderr = []
  child.stdout.setEncoding('utf8')
  child.stderr.setEncoding('utf8')
  child.stdout.on('data', (chunk) => stdout.push(chunk))
  child.stderr.on('data', (chunk) => stderr.push(chunk))
  child.stdin.end(`${JSON.stringify(request)}\n`)
  const [code] = await new Promise((resolve, reject) => {
    child.once('error', reject)
    child.once('close', (...result) => resolve(result))
  })
  assert.equal(code, 0, stderr.join(''))
  return JSON.parse(stdout.join('').trim())
}

test('terminates and reaps a frontmost activation child that ignores its deadline', async () => {
  const startedAt = Date.now()
  const result = await runJsonMode(['--protocol-test-bounded-frontmost-child'])

  assert.deepEqual(result, {
    timedOut: true,
    terminated: true,
    killed: true,
    reaped: true,
  })
  assert.ok(Date.now() - startedAt < 1_000)
})

test('prepares focus without global pointer event injection', async () => {
  const source = await readFile(sourcePath, 'utf8')
  const prepareFocus = source.match(
    /func prepareFocus\([\s\S]*?\n    func perform\(/g,
  )?.at(-1)

  assert.ok(prepareFocus)
  assert.doesNotMatch(prepareFocus, /CGEvent\(|cghidEventTap|leftMouseDown|leftMouseUp/)
})

test('rejects a post-activation CG geometry change before focus dispatch', async () => {
  const response = await runRequest(
    ['--protocol-test', '--protocol-test-mac-geometry-change-fixture'],
    {
      version: 1,
      sequence: 1,
      pid: 101,
      windowId: 44,
      command: 'focus',
      timeoutMs: 100,
      payload: { target: { role: 'AXWindow' } },
    },
  )

  assert.equal(response.ok, false)
  assert.equal(response.error.code, 'PRECONDITION_WINDOW_OWNER')
  assert.equal(response.error.message, 'Target window changed')
})

test('requires exact frame geometry even when AXWindowNumber matches', async () => {
  const result = await runJsonMode(['--protocol-test-ax-window-identity'])

  assert.deepEqual(result, {
    exactNumberExactFrame: true,
    exactNumberChangedFrame: false,
    noNumberExactFrame: true,
  })
})
