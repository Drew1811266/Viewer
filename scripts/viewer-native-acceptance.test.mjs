import assert from 'node:assert/strict'
import { execFile, spawn } from 'node:child_process'
import {
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  stat,
  symlink,
  writeFile,
} from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { describe, it } from 'node:test'
import { promisify } from 'node:util'

import {
  ALLOWED_COMMANDS,
  NativeAcceptanceClient,
  PROTOCOL_VERSION,
  buildNativeHelper,
  parseProcessTable,
  selectExactViewer,
  selectExactWindow,
  validateCommand,
  validateEvidencePath,
  validateFixturePath,
  validateWindow,
  validateWindowPoint,
} from './viewer-native-acceptance.mjs'

const repoRoot = '/Users/example/Project/Viewer/.worktrees/atlas'
const executablePath = `${repoRoot}/target/debug/viewer-desktop`
const execFileAsync = promisify(execFile)

async function runJsonLines(executable, args, requests) {
  const child = spawn(executable, args, { stdio: ['pipe', 'pipe', 'pipe'] })
  child.stdout.setEncoding('utf8')
  child.stderr.setEncoding('utf8')
  let stdout = ''
  let stderr = ''
  child.stdout.on('data', (chunk) => {
    stdout += chunk
  })
  child.stderr.on('data', (chunk) => {
    stderr += chunk
  })
  for (const request of requests) child.stdin.write(`${JSON.stringify(request)}\n`)
  child.stdin.end()
  const exitCode = await new Promise((resolve, reject) => {
    child.once('error', reject)
    child.once('close', resolve)
  })
  return {
    exitCode,
    stderr,
    responses: stdout
      .trim()
      .split('\n')
      .filter(Boolean)
      .map((line) => JSON.parse(line)),
  }
}

async function createProtocolFixture(temporaryRoot) {
  const fixturePath = path.join(temporaryRoot, 'protocol-fixture.mjs')
  await writeFile(
    fixturePath,
    `import { appendFileSync } from 'node:fs'
import readline from 'node:readline'

const mode = process.argv[2]
const input = readline.createInterface({ input: process.stdin })
input.on('line', (line) => {
  const request = JSON.parse(line)
  if (process.env.REQUEST_LOG) {
    appendFileSync(process.env.REQUEST_LOG, JSON.stringify(request) + '\\n')
  }
  if (mode === 'timeout') return
  if (mode === 'eof') process.exit(0)
  if (mode === 'malformed') {
    process.stdout.write('{not-json}\\n')
    return
  }
  if (mode === 'stderr') {
    process.stderr.write('fixture diagnostic\\n')
    process.exit(3)
  }
  const sequence = mode === 'out-of-order' ? request.sequence + 1 : request.sequence
  process.stdout.write(JSON.stringify({
    version: 1,
    sequence,
    ok: true,
    result: { mode: 'fixture', command: request.command },
  }) + '\\n')
  if (request.command === 'shutdown') process.exit(0)
})
`,
  )
  return fixturePath
}

describe('selectExactViewer', () => {
  it('returns the only exact current-worktree bare development process', () => {
    const processes = parseProcessTable(`
      101 1 101 ${executablePath}
    `)

    assert.deepEqual(processes, [
      {
        pid: 101,
        ppid: 1,
        pgid: 101,
        command: executablePath,
      },
    ])
    assert.equal(
      selectExactViewer(processes, {
        repoRoot,
        executablePath,
        controllerPid: 999,
      }).pid,
      101,
    )
  })

  it('rejects a missing Viewer process', () => {
    assert.throws(
      () =>
        selectExactViewer([], {
          repoRoot,
          executablePath,
          controllerPid: 999,
        }),
      { code: 'PRECONDITION_VIEWER_COUNT' },
    )
  })

  it('rejects multiple Viewer processes across repository worktrees', () => {
    const processes = parseProcessTable(`
      101 1 101 ${executablePath}
      202 1 202 /Users/example/Project/Viewer/target/debug/viewer-desktop
    `)

    assert.throws(
      () =>
        selectExactViewer(processes, {
          repoRoot,
          executablePath,
          controllerPid: 999,
        }),
      { code: 'PRECONDITION_VIEWER_COUNT' },
    )
  })

  it('rejects a packaged Viewer process', () => {
    const processes = parseProcessTable(`
      101 1 101 ${repoRoot}/target/debug/bundle/macos/Viewer.app/Contents/MacOS/viewer-desktop
    `)

    assert.throws(
      () =>
        selectExactViewer(processes, {
          repoRoot,
          executablePath,
          controllerPid: 999,
        }),
      { code: 'PRECONDITION_VIEWER_PATH' },
    )
  })

  it('rejects a lone Viewer process from another worktree', () => {
    const processes = parseProcessTable(`
      101 1 101 /Users/example/Project/Viewer/.worktrees/other/target/debug/viewer-desktop
    `)

    assert.throws(
      () =>
        selectExactViewer(processes, {
          repoRoot,
          executablePath,
          controllerPid: 999,
        }),
      { code: 'PRECONDITION_VIEWER_PATH' },
    )
  })

  it('rejects the controller or helper process as the Viewer target', () => {
    const processes = parseProcessTable(`
      101 1 101 ${executablePath}
    `)

    assert.throws(
      () =>
        selectExactViewer(processes, {
          repoRoot,
          executablePath,
          controllerPid: 101,
        }),
      { code: 'PRECONDITION_VIEWER_SELF' },
    )
    assert.throws(
      () =>
        selectExactViewer(processes, {
          repoRoot,
          executablePath,
          controllerPid: 999,
          helperPid: 101,
        }),
      { code: 'PRECONDITION_VIEWER_SELF' },
    )
  })
})

describe('window validation', () => {
  const window = {
    pid: 101,
    windowId: 44,
    x: 20,
    y: 30,
    width: 1024,
    height: 720,
  }

  it('accepts one exact owned acceptance window', () => {
    assert.deepEqual(
      selectExactWindow([window], {
        pid: 101,
        viewport: { width: 1024, height: 720 },
      }),
      window,
    )
  })

  it('rejects missing and duplicate main windows', () => {
    assert.throws(
      () =>
        selectExactWindow([], {
          pid: 101,
          viewport: { width: 1024, height: 720 },
        }),
      { code: 'PRECONDITION_WINDOW_COUNT' },
    )
    assert.throws(
      () =>
        selectExactWindow([window, { ...window, windowId: 45 }], {
          pid: 101,
          viewport: { width: 1024, height: 720 },
        }),
      { code: 'PRECONDITION_WINDOW_COUNT' },
    )
  })

  it('rejects a window owned by another process', () => {
    assert.throws(
      () =>
        validateWindow({ ...window, pid: 202 }, {
          pid: 101,
          viewport: { width: 1024, height: 720 },
        }),
      { code: 'PRECONDITION_WINDOW_OWNER' },
    )
  })

  it('rejects a non-positive or non-integer window ID', () => {
    for (const windowId of [0, -1, 44.5, Number.NaN]) {
      assert.throws(
        () =>
          validateWindow({ ...window, windowId }, {
            pid: 101,
            viewport: { width: 1024, height: 720 },
          }),
        { code: 'PRECONDITION_WINDOW_OWNER' },
      )
    }
  })

  it('rejects dimensions outside the two exact acceptance viewports', () => {
    for (const candidate of [
      { ...window, width: 1023 },
      { ...window, width: 1440, height: 899 },
      { ...window, width: 720, height: 450 },
    ]) {
      assert.throws(
        () =>
          validateWindow(candidate, {
            pid: 101,
            viewport: { width: candidate.width, height: candidate.height },
          }),
        { code: 'PRECONDITION_VIEWPORT' },
      )
    }
  })

  it('accepts exact 1440 by 900 dimensions', () => {
    const largeWindow = { ...window, width: 1440, height: 900 }
    assert.equal(
      validateWindow(largeWindow, {
        pid: 101,
        viewport: { width: 1440, height: 900 },
      }),
      largeWindow,
    )
  })
})

describe('coordinate validation', () => {
  const window = {
    pid: 101,
    windowId: 44,
    x: 20,
    y: 30,
    width: 1024,
    height: 720,
  }

  it('translates an in-window relative point to screen coordinates', () => {
    assert.deepEqual(validateWindowPoint({ x: 0, y: 0 }, window), {
      x: 20,
      y: 30,
    })
    assert.deepEqual(validateWindowPoint({ x: 1023, y: 719 }, window), {
      x: 1043,
      y: 749,
    })
  })

  it('rejects non-finite, negative and right-or-bottom-edge points', () => {
    for (const point of [
      { x: -1, y: 0 },
      { x: 0, y: -1 },
      { x: 1024, y: 0 },
      { x: 0, y: 720 },
      { x: Number.NaN, y: 1 },
      { x: 1, y: Number.POSITIVE_INFINITY },
    ]) {
      assert.throws(() => validateWindowPoint(point, window), {
        code: 'SAFETY_POINT_OUTSIDE_WINDOW',
      })
    }
  })
})

describe('fixture path validation', () => {
  it('accepts only the current disposable run root and descendants', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-acceptance-'))
    const runId = 'run-123'
    const runRoot = path.join(
      temporaryRoot,
      'target',
      'atlas-product-migration-fixture',
      'runs',
      runId,
    )
    const child = path.join(runRoot, 'project', '衣服', 'A01')

    try {
      await mkdir(child, { recursive: true })
      assert.equal(
        await validateFixturePath(runRoot, { repoRoot: temporaryRoot, runId }),
        runRoot,
      )
      assert.equal(
        await validateFixturePath(child, { repoRoot: temporaryRoot, runId }),
        child,
      )
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })

  it('rejects the baseline, repository root, sibling runs and parent escape', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-acceptance-'))
    const runId = 'run-123'
    const fixtureRoot = path.join(
      temporaryRoot,
      'target',
      'atlas-product-migration-fixture',
    )
    const runRoot = path.join(fixtureRoot, 'runs', runId)
    const rejected = [
      '/',
      temporaryRoot,
      path.join(fixtureRoot, 'ViewerAcceptance'),
      path.join(fixtureRoot, 'runs', 'run-456'),
      path.join(runRoot, '..', 'run-456'),
      '$HOME/project',
      '~/project',
      path.join(runRoot, '*.jpg'),
    ]

    try {
      await mkdir(path.join(fixtureRoot, 'ViewerAcceptance'), { recursive: true })
      await mkdir(runRoot, { recursive: true })
      await mkdir(path.join(fixtureRoot, 'runs', 'run-456'), { recursive: true })

      for (const candidate of rejected) {
        await assert.rejects(
          validateFixturePath(candidate, { repoRoot: temporaryRoot, runId }),
          { code: 'SAFETY_FIXTURE_PATH' },
        )
      }
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })

  it('rejects a symbolic-link escape from the approved run', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-acceptance-'))
    const outside = await mkdtemp(path.join(os.tmpdir(), 'viewer-acceptance-outside-'))
    const runId = 'run-123'
    const runRoot = path.join(
      temporaryRoot,
      'target',
      'atlas-product-migration-fixture',
      'runs',
      runId,
    )
    const link = path.join(runRoot, 'escape')

    try {
      await mkdir(runRoot, { recursive: true })
      await writeFile(path.join(outside, 'secret.txt'), 'outside\n')
      await symlink(outside, link)

      await assert.rejects(
        validateFixturePath(path.join(link, 'secret.txt'), {
          repoRoot: temporaryRoot,
          runId,
        }),
        { code: 'SAFETY_FIXTURE_PATH' },
      )
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
      await rm(outside, { recursive: true, force: true })
    }
  })
})

describe('evidence path validation', () => {
  it('accepts only the exact commit, viewport and ledger-ID directory', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-evidence-'))
    const context = {
      repoRoot: temporaryRoot,
      commit: 'abc123',
      viewport: '1024x720',
      id: 'FIL-01',
    }
    const evidenceRoot = path.join(
      temporaryRoot,
      'target',
      'atlas-product-migration-acceptance',
      'abc123',
      '1024x720',
      'FIL-01',
    )
    const productPath = path.join(evidenceRoot, 'product.png')

    try {
      await mkdir(evidenceRoot, { recursive: true })
      assert.equal(await validateEvidencePath(productPath, context), productPath)

      for (const candidate of [
        path.join(evidenceRoot, '..', 'FIL-02', 'product.png'),
        path.join(evidenceRoot, '..', '..', '1440x900', 'FIL-01', 'product.png'),
        path.join(evidenceRoot, '..', '..', '..', 'def456', '1024x720', 'FIL-01', 'product.png'),
        path.join(temporaryRoot, 'product.png'),
      ]) {
        await assert.rejects(validateEvidencePath(candidate, context), {
          code: 'SAFETY_EVIDENCE_PATH',
        })
      }
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })
})

describe('protocol schema', () => {
  const window = {
    pid: 101,
    windowId: 44,
    x: 20,
    y: 30,
    width: 1024,
    height: 720,
  }
  const inspectRequest = {
    version: 1,
    sequence: 1,
    pid: 101,
    windowId: 44,
    command: 'inspect',
    timeoutMs: 3000,
    payload: {},
  }

  it('accepts the complete strict inspect envelope', () => {
    assert.equal(PROTOCOL_VERSION, 1)
    assert.deepEqual(
      [...ALLOWED_COMMANDS],
      [
        'inspect',
        'query',
        'activate',
        'focus',
        'setValue',
        'key',
        'pointer',
        'drag',
        'capture',
        'shutdown',
      ],
    )
    assert.deepEqual(validateCommand(inspectRequest, { window }), inspectRequest)
  })

  it('rejects invalid common envelope values and extra fields', () => {
    const cases = [
      { ...inspectRequest, version: 2 },
      { ...inspectRequest, sequence: 0 },
      { ...inspectRequest, sequence: 1.5 },
      { ...inspectRequest, pid: 0 },
      { ...inspectRequest, windowId: 0 },
      { ...inspectRequest, command: 'shell' },
      { ...inspectRequest, timeoutMs: 99 },
      { ...inspectRequest, timeoutMs: 10001 },
      { ...inspectRequest, extra: true },
    ]

    for (const request of cases) {
      assert.throws(() => validateCommand(request, { window }), {
        code: 'SAFETY_PROTOCOL',
      })
    }
  })

  it('rejects extra payload fields for an otherwise valid command', () => {
    assert.throws(
      () =>
        validateCommand(
          { ...inspectRequest, payload: { ignored: true } },
          { window },
        ),
      { code: 'SAFETY_COMMAND' },
    )
  })

  it('accepts bounded setValue text and rejects text above 4096 scalars', () => {
    const request = {
      ...inspectRequest,
      command: 'setValue',
      payload: {
        target: { role: 'AXTextField', name: '搜索' },
        text: '衣服/A01',
      },
    }
    assert.deepEqual(validateCommand(request, { window }), request)
    assert.throws(
      () =>
        validateCommand(
          {
            ...request,
            payload: { ...request.payload, text: '图'.repeat(4097) },
          },
          { window },
        ),
      { code: 'SAFETY_COMMAND' },
    )
  })

  it('accepts only the approved key and modifier vocabulary', () => {
    const request = {
      ...inspectRequest,
      command: 'key',
      payload: { key: 'escape', modifiers: ['shift', 'command'] },
    }
    assert.deepEqual(validateCommand(request, { window }), request)

    for (const payload of [
      { key: 'f1', modifiers: [] },
      { key: 'escape', modifiers: ['fn'] },
      { key: 'escape', modifiers: ['shift', 'shift'] },
      { key: 'escape', modifiers: [], script: 'rm' },
    ]) {
      assert.throws(
        () => validateCommand({ ...request, payload }, { window }),
        { code: 'SAFETY_COMMAND' },
      )
    }
  })

  it('validates pointer and drag coordinates against the current window', () => {
    const pointerRequest = {
      ...inspectRequest,
      command: 'pointer',
      payload: { kind: 'rightClick', point: { x: 100, y: 200 } },
    }
    const dragRequest = {
      ...inspectRequest,
      command: 'drag',
      payload: {
        from: { x: 100, y: 200 },
        to: { x: 300, y: 400 },
        durationMs: 400,
      },
    }
    assert.deepEqual(validateCommand(pointerRequest, { window }), pointerRequest)
    assert.deepEqual(validateCommand(dragRequest, { window }), dragRequest)

    assert.throws(
      () =>
        validateCommand(
          {
            ...pointerRequest,
            payload: { kind: 'click', point: { x: 1024, y: 10 } },
          },
          { window },
        ),
      { code: 'SAFETY_COMMAND' },
    )
    assert.throws(
      () =>
        validateCommand(
          {
            ...dragRequest,
            payload: { ...dragRequest.payload, durationMs: 5001 },
          },
          { window },
        ),
      { code: 'SAFETY_COMMAND' },
    )
  })
})

describe('Swift helper protocol', () => {
  it('compiles warning-free and correlates valid and rejected commands', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-swift-helper-'))
    const helperPath = path.join(temporaryRoot, 'viewer-native-acceptance-helper')
    const inspect = {
      version: 1,
      sequence: 1,
      pid: 101,
      windowId: 44,
      command: 'inspect',
      timeoutMs: 3000,
      payload: {},
    }
    const unknown = { ...inspect, sequence: 2, command: 'shell' }

    try {
      await execFileAsync('xcrun', [
        'swiftc',
        '-warnings-as-errors',
        new URL('./viewer-native-acceptance.swift', import.meta.url).pathname,
        '-o',
        helperPath,
      ])
      const result = await runJsonLines(helperPath, ['--protocol-test'], [inspect, unknown])

      assert.equal(result.exitCode, 0)
      assert.equal(result.stderr, '')
      assert.deepEqual(result.responses, [
        {
          version: 1,
          sequence: 1,
          ok: true,
          result: { mode: 'protocol-test' },
        },
        {
          version: 1,
          sequence: 2,
          ok: false,
          error: { code: 'SAFETY_COMMAND', message: 'Unsupported command' },
        },
      ])
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })
})

describe('NativeAcceptanceClient', () => {
  const window = {
    pid: 101,
    windowId: 44,
    x: 20,
    y: 30,
    width: 1024,
    height: 720,
  }

  it('correlates ordered requests and closes with shutdown', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-client-'))
    const requestLog = path.join(temporaryRoot, 'requests.jsonl')
    let client

    try {
      const fixturePath = await createProtocolFixture(temporaryRoot)
      client = new NativeAcceptanceClient({
        executablePath: process.execPath,
        args: [fixturePath, 'valid'],
        env: { ...process.env, REQUEST_LOG: requestLog },
        pid: 101,
        window,
        defaultTimeoutMs: 500,
      })

      assert.deepEqual(await client.start(), { mode: 'fixture', command: 'inspect' })
      assert.deepEqual(
        await client.request('query', {
          target: { role: 'AXButton', name: '筛选' },
        }),
        { mode: 'fixture', command: 'query' },
      )
      await client.close()
      client = undefined

      const requests = (await readFile(requestLog, 'utf8'))
        .trim()
        .split('\n')
        .map((line) => JSON.parse(line))
      assert.deepEqual(
        requests.map(({ sequence, command }) => ({ sequence, command })),
        [
          { sequence: 1, command: 'inspect' },
          { sequence: 2, command: 'query' },
          { sequence: 3, command: 'shutdown' },
        ],
      )
    } finally {
      await client?.terminate()
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })

  for (const [mode, code] of [
    ['out-of-order', 'SAFETY_PROTOCOL'],
    ['malformed', 'SAFETY_PROTOCOL'],
    ['eof', 'PRECONDITION_HELPER_EXIT'],
    ['timeout', 'PRECONDITION_HELPER_TIMEOUT'],
  ]) {
    it(`rejects ${mode} helper behavior`, async () => {
      const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-client-'))
      let client
      try {
        const fixturePath = await createProtocolFixture(temporaryRoot)
        client = new NativeAcceptanceClient({
          executablePath: process.execPath,
          args: [fixturePath, mode],
          pid: 101,
          window,
          defaultTimeoutMs: 150,
        })
        await assert.rejects(client.start(), { code })
      } finally {
        await client?.terminate()
        await rm(temporaryRoot, { recursive: true, force: true })
      }
    })
  }

  it('preserves helper stderr when the child exits', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-client-'))
    let client
    try {
      const fixturePath = await createProtocolFixture(temporaryRoot)
      client = new NativeAcceptanceClient({
        executablePath: process.execPath,
        args: [fixturePath, 'stderr'],
        pid: 101,
        window,
        defaultTimeoutMs: 500,
      })
      await assert.rejects(client.start(), (error) => {
        assert.equal(error.code, 'PRECONDITION_HELPER_EXIT')
        assert.match(error.details.stderr, /fixture diagnostic/)
        return true
      })
    } finally {
      await client?.terminate()
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })
})

describe('native helper build cache', () => {
  it('keys the warning-free Swift executable by the complete source hash', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-helper-build-'))
    const sourcePath = path.join(temporaryRoot, 'viewer-native-acceptance.swift')
    try {
      await copyFile(
        new URL('./viewer-native-acceptance.swift', import.meta.url),
        sourcePath,
      )
      const first = await buildNativeHelper({
        repoRoot: temporaryRoot,
        sourcePath,
      })
      const firstModifiedAt = (await stat(first.executablePath)).mtimeMs
      const second = await buildNativeHelper({
        repoRoot: temporaryRoot,
        sourcePath,
      })

      assert.deepEqual(second, first)
      assert.equal((await stat(second.executablePath)).mtimeMs, firstModifiedAt)
      assert.match(
        first.executablePath,
        /target\/native-acceptance-tools\/[0-9a-f]{64}\/viewer-native-acceptance-helper$/,
      )

      await writeFile(sourcePath, `${await readFile(sourcePath, 'utf8')}\n`)
      const changed = await buildNativeHelper({
        repoRoot: temporaryRoot,
        sourcePath,
      })
      assert.notEqual(changed.sourceHash, first.sourceHash)
      assert.notEqual(changed.executablePath, first.executablePath)
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })
})

describe('native validation', () => {
  it('shares PID, window, unique-target and coordinate guards with the fixture adapter', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-native-fixture-'))
    const helperPath = path.join(temporaryRoot, 'viewer-native-acceptance-helper')
    const base = {
      version: 1,
      sequence: 1,
      pid: 101,
      windowId: 44,
      command: 'inspect',
      timeoutMs: 3000,
      payload: {},
    }
    const requests = [
      base,
      { ...base, sequence: 2, pid: 202 },
      {
        ...base,
        sequence: 3,
        command: 'query',
        payload: { target: { role: 'AXButton', name: '筛选' } },
      },
      {
        ...base,
        sequence: 4,
        command: 'query',
        payload: { target: { role: 'AXButton', name: '重复' } },
      },
      {
        ...base,
        sequence: 5,
        command: 'query',
        payload: { target: { role: 'AXButton', name: '不存在' } },
      },
      {
        ...base,
        sequence: 6,
        command: 'pointer',
        payload: { kind: 'click', point: { x: 1024, y: 10 } },
      },
    ]

    try {
      await execFileAsync('xcrun', [
        'swiftc',
        '-warnings-as-errors',
        new URL('./viewer-native-acceptance.swift', import.meta.url).pathname,
        '-o',
        helperPath,
      ])
      const result = await runJsonLines(
        helperPath,
        ['--protocol-test', '--protocol-test-fixture'],
        requests,
      )

      assert.equal(result.exitCode, 0)
      assert.equal(result.stderr, '')
      assert.deepEqual(result.responses[0], {
        version: 1,
        sequence: 1,
        ok: true,
        result: {
          pid: 101,
          windowId: 44,
          title: 'Viewer Acceptance Fixture',
          frame: { x: 20, y: 30, width: 1024, height: 720 },
          focused: true,
          frontmost: true,
        },
      })
      assert.deepEqual(result.responses[1], {
        version: 1,
        sequence: 2,
        ok: false,
        error: {
          code: 'PRECONDITION_WINDOW_OWNER',
          message: 'Target window changed',
        },
      })
      assert.deepEqual(result.responses[2], {
        version: 1,
        sequence: 3,
        ok: true,
        result: {
          elements: [
            {
              role: 'AXButton',
              name: '筛选',
              identifier: 'toolbar-filter',
              enabled: true,
              focused: false,
              frame: { x: 900, y: 40, width: 80, height: 32 },
              path: [0, 0],
            },
          ],
        },
      })
      assert.deepEqual(
        result.responses.slice(3).map((response) => response.error.code),
        [
          'STATE_TARGET_NOT_UNIQUE',
          'STATE_TARGET_NOT_FOUND',
          'SAFETY_POINT_OUTSIDE_WINDOW',
        ],
      )
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })

  it('validates every action payload without emitting events in fixture mode', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-native-actions-'))
    const helperPath = path.join(temporaryRoot, 'viewer-native-acceptance-helper')
    const base = {
      version: 1,
      sequence: 1,
      pid: 101,
      windowId: 44,
      command: 'inspect',
      timeoutMs: 3000,
      payload: {},
    }
    const requests = [
      {
        ...base,
        sequence: 1,
        command: 'activate',
        payload: { target: { role: 'AXButton', name: '筛选' } },
      },
      {
        ...base,
        sequence: 2,
        command: 'focus',
        payload: { target: { identifier: 'toolbar-filter' } },
      },
      {
        ...base,
        sequence: 3,
        command: 'setValue',
        payload: {
          target: { role: 'AXTextField', name: '搜索' },
          text: '衣服/A01',
        },
      },
      {
        ...base,
        sequence: 4,
        command: 'key',
        payload: { key: 'escape', modifiers: ['shift'] },
      },
      {
        ...base,
        sequence: 5,
        command: 'pointer',
        payload: { kind: 'rightClick', point: { x: 100, y: 200 } },
      },
      {
        ...base,
        sequence: 6,
        command: 'drag',
        payload: {
          from: { x: 100, y: 200 },
          to: { x: 300, y: 400 },
          durationMs: 400,
        },
      },
      {
        ...base,
        sequence: 7,
        command: 'capture',
        payload: { path: '/tmp/viewer-acceptance-product.png' },
      },
      {
        ...base,
        sequence: 8,
        command: 'shutdown',
        payload: {},
      },
      {
        ...base,
        sequence: 9,
        command: 'setValue',
        payload: {
          target: { role: 'AXTextField', name: '搜索' },
          text: '图'.repeat(4097),
        },
      },
      {
        ...base,
        sequence: 10,
        command: 'drag',
        payload: {
          from: { x: 100, y: 200 },
          to: { x: 1024, y: 400 },
          durationMs: 400,
        },
      },
    ]

    try {
      await execFileAsync('xcrun', [
        'swiftc',
        '-warnings-as-errors',
        new URL('./viewer-native-acceptance.swift', import.meta.url).pathname,
        '-o',
        helperPath,
      ])
      const result = await runJsonLines(
        helperPath,
        ['--protocol-test', '--protocol-test-fixture'],
        requests,
      )

      assert.equal(result.exitCode, 0)
      assert.equal(result.stderr, '')
      assert.deepEqual(
        result.responses.slice(0, 8).map((response) => ({
          ok: response.ok,
          performed: response.result.performed,
          command: response.result.command,
        })),
        [
          { ok: true, performed: true, command: 'activate' },
          { ok: true, performed: true, command: 'focus' },
          { ok: true, performed: true, command: 'setValue' },
          { ok: true, performed: true, command: 'key' },
          { ok: true, performed: true, command: 'pointer' },
          { ok: true, performed: true, command: 'drag' },
          { ok: true, performed: true, command: 'capture' },
          { ok: true, performed: true, command: 'shutdown' },
        ],
      )
      assert.deepEqual(
        result.responses.slice(8).map((response) => response.error.code),
        ['SAFETY_COMMAND', 'SAFETY_POINT_OUTSIDE_WINDOW'],
      )
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })
})

describe('native production guard', () => {
  it('rejects an invalid Viewer PID before attempting a native action', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-native-guard-'))
    const helperPath = path.join(temporaryRoot, 'viewer-native-acceptance-helper')
    const request = {
      version: 1,
      sequence: 1,
      pid: 2_147_483_647,
      windowId: 1,
      command: 'inspect',
      timeoutMs: 3000,
      payload: {},
    }

    try {
      await execFileAsync('xcrun', [
        'swiftc',
        '-warnings-as-errors',
        new URL('./viewer-native-acceptance.swift', import.meta.url).pathname,
        '-o',
        helperPath,
      ])
      const result = await runJsonLines(helperPath, [], [request])

      assert.equal(result.exitCode, 0)
      assert.equal(result.stderr, '')
      assert.deepEqual(result.responses, [
        {
          version: 1,
          sequence: 1,
          ok: false,
          error: {
            code: 'PRECONDITION_VIEWER_PID',
            message: 'Viewer PID is not running',
          },
        },
      ])
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })
})

describe('MacAdapter command dispatch', () => {
  it('routes every validated command through the recording system adapter', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-mac-adapter-'))
    const helperPath = path.join(temporaryRoot, 'viewer-native-acceptance-helper')
    const base = {
      version: 1,
      sequence: 1,
      pid: 101,
      windowId: 44,
      command: 'inspect',
      timeoutMs: 3000,
      payload: {},
    }
    const requests = [
      base,
      {
        ...base,
        sequence: 2,
        command: 'query',
        payload: { target: { identifier: 'toolbar-filter' } },
      },
      {
        ...base,
        sequence: 3,
        command: 'activate',
        payload: { target: { identifier: 'toolbar-filter' } },
      },
      {
        ...base,
        sequence: 4,
        command: 'focus',
        payload: { target: { identifier: 'toolbar-search' } },
      },
      {
        ...base,
        sequence: 5,
        command: 'setValue',
        payload: {
          target: { identifier: 'toolbar-search' },
          text: '衣服/A01',
        },
      },
      {
        ...base,
        sequence: 6,
        command: 'key',
        payload: { key: 'escape', modifiers: [] },
      },
      {
        ...base,
        sequence: 7,
        command: 'pointer',
        payload: { kind: 'click', point: { x: 100, y: 200 } },
      },
      {
        ...base,
        sequence: 8,
        command: 'drag',
        payload: {
          from: { x: 100, y: 200 },
          to: { x: 300, y: 400 },
          durationMs: 400,
        },
      },
      {
        ...base,
        sequence: 9,
        command: 'capture',
        payload: { path: '/tmp/viewer-mac-adapter.png' },
      },
      { ...base, sequence: 10, command: 'shutdown', payload: {} },
    ]

    try {
      await execFileAsync('xcrun', [
        'swiftc',
        '-warnings-as-errors',
        new URL('./viewer-native-acceptance.swift', import.meta.url).pathname,
        '-o',
        helperPath,
      ])
      const result = await runJsonLines(
        helperPath,
        ['--protocol-test', '--protocol-test-mac-fixture'],
        requests,
      )

      assert.equal(result.exitCode, 0)
      assert.equal(result.stderr, '')
      assert.deepEqual(result.responses[0].result, {
        pid: 101,
        windowId: 44,
        title: 'Viewer Mac Adapter Fixture',
        frame: { x: 20, y: 30, width: 1024, height: 720 },
        focused: true,
        frontmost: true,
      })
      assert.deepEqual(result.responses[1].result.elements, [
        {
          role: 'AXButton',
          name: '筛选',
          identifier: 'toolbar-filter',
          enabled: true,
          focused: false,
          frame: { x: 900, y: 40, width: 80, height: 32 },
          path: [0, 0],
        },
      ])
      assert.deepEqual(
        result.responses.slice(2, 8).map((response) => response.result.command),
        ['activate', 'focus', 'setValue', 'key', 'pointer', 'drag'],
      )
      assert.deepEqual(result.responses[8].result, {
        command: 'capture',
        performed: true,
        width: 2048,
        height: 1440,
        sha256: 'a'.repeat(64),
      })
      assert.deepEqual(result.responses[9].result, {
        command: 'shutdown',
        performed: true,
      })
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })

  it('focuses an owned background Viewer window before guarded input', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-mac-focus-'))
    const helperPath = path.join(temporaryRoot, 'viewer-native-acceptance-helper')
    const base = {
      version: 1,
      sequence: 1,
      pid: 101,
      windowId: 44,
      command: 'inspect',
      timeoutMs: 3000,
      payload: {},
    }
    const requests = [
      base,
      {
        ...base,
        sequence: 2,
        command: 'focus',
        payload: { target: { role: 'AXWindow', name: 'Viewer Mac Adapter Fixture' } },
      },
      { ...base, sequence: 3 },
      {
        ...base,
        sequence: 4,
        command: 'pointer',
        payload: { kind: 'click', point: { x: 100, y: 200 } },
      },
    ]

    try {
      await execFileAsync('xcrun', [
        'swiftc',
        '-warnings-as-errors',
        new URL('./viewer-native-acceptance.swift', import.meta.url).pathname,
        '-o',
        helperPath,
      ])
      const result = await runJsonLines(
        helperPath,
        ['--protocol-test', '--protocol-test-mac-background-fixture'],
        requests,
      )

      assert.equal(result.exitCode, 0)
      assert.equal(result.stderr, '')
      assert.equal(result.responses[0].result.frontmost, false)
      assert.deepEqual(result.responses[1].result, {
        performed: true,
        command: 'focus',
      })
      assert.equal(result.responses[2].result.frontmost, true)
      assert.deepEqual(result.responses[3].result, {
        performed: true,
        command: 'pointer',
      })
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })
})
