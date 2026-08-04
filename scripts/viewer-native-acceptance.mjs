import path from 'node:path'
import { execFile, spawn } from 'node:child_process'
import { createHash, randomUUID } from 'node:crypto'
import { access, lstat, mkdir, readFile, realpath, rename, rm } from 'node:fs/promises'
import readline from 'node:readline'
import { promisify } from 'node:util'

const execFileAsync = promisify(execFile)

export const PROTOCOL_VERSION = 1
export const ALLOWED_COMMANDS = new Set([
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
])

const ALLOWED_KEYS = new Set([
  'tab',
  'enter',
  'space',
  'escape',
  'arrowUp',
  'arrowDown',
  'arrowLeft',
  'arrowRight',
  'home',
  'end',
  'delete',
  'backspace',
  'a',
  'c',
  'v',
])
const ALLOWED_MODIFIERS = new Set(['shift', 'control', 'option', 'command'])

export class AcceptanceError extends Error {
  constructor(code, message, details = {}) {
    super(message)
    this.name = 'AcceptanceError'
    this.code = code
    this.details = details
  }
}

function commandExecutable(command) {
  return command.trim().split(/\s+/, 1)[0]
}

export function parseProcessTable(output) {
  return output
    .split('\n')
    .map((line) => line.match(/^\s*(\d+)\s+(\d+)\s+(\d+)\s+(.+?)\s*$/))
    .filter((match) => match !== null)
    .map((match) => ({
      pid: Number(match[1]),
      ppid: Number(match[2]),
      pgid: Number(match[3]),
      command: match[4],
    }))
}

function isViewerCandidate(processInfo) {
  return path.basename(commandExecutable(processInfo.command)) === 'viewer-desktop'
}

export function selectExactViewer(
  processes,
  { executablePath, controllerPid, helperPid },
) {
  const candidates = processes.filter(isViewerCandidate)

  if (candidates.length === 0) {
    throw new AcceptanceError(
      'PRECONDITION_VIEWER_COUNT',
      'Expected exactly one Viewer process, found none',
      { candidatePids: [] },
    )
  }

  if (candidates.length > 1) {
    throw new AcceptanceError(
      'PRECONDITION_VIEWER_COUNT',
      `Expected exactly one Viewer process, found ${candidates.length}`,
      { candidatePids: candidates.map((item) => item.pid) },
    )
  }

  const viewer = candidates[0]
  if (commandExecutable(viewer.command) !== executablePath) {
    throw new AcceptanceError(
      'PRECONDITION_VIEWER_PATH',
      'Viewer executable is not the exact current-worktree development binary',
      { actual: commandExecutable(viewer.command), expected: executablePath },
    )
  }

  if (viewer.pid === controllerPid || viewer.pid === helperPid) {
    throw new AcceptanceError(
      'PRECONDITION_VIEWER_SELF',
      'Controller or helper process cannot be selected as Viewer',
      { pid: viewer.pid },
    )
  }

  return viewer
}

function isAllowedViewport(viewport) {
  return (
    (viewport?.width === 1024 && viewport?.height === 720) ||
    (viewport?.width === 1440 && viewport?.height === 900)
  )
}

export function validateWindow(window, { pid, viewport }) {
  if (
    window?.pid !== pid ||
    !Number.isInteger(window?.windowId) ||
    window.windowId <= 0
  ) {
    throw new AcceptanceError(
      'PRECONDITION_WINDOW_OWNER',
      'Viewer window is not owned by the approved process',
      { actualPid: window?.pid, expectedPid: pid, windowId: window?.windowId },
    )
  }

  if (
    !isAllowedViewport(viewport) ||
    window.width !== viewport.width ||
    window.height !== viewport.height
  ) {
    throw new AcceptanceError(
      'PRECONDITION_VIEWPORT',
      'Viewer window does not match an exact acceptance viewport',
      {
        actual: { width: window.width, height: window.height },
        expected: viewport,
      },
    )
  }

  return window
}

export function selectExactWindow(windows, options) {
  if (windows.length !== 1) {
    throw new AcceptanceError(
      'PRECONDITION_WINDOW_COUNT',
      `Expected exactly one Viewer main window, found ${windows.length}`,
      { windowIds: windows.map((window) => window.windowId) },
    )
  }

  return validateWindow(windows[0], options)
}

export function validateWindowPoint(point, window) {
  if (
    !Number.isFinite(point?.x) ||
    !Number.isFinite(point?.y) ||
    point.x < 0 ||
    point.y < 0 ||
    point.x >= window.width ||
    point.y >= window.height
  ) {
    throw new AcceptanceError(
      'SAFETY_POINT_OUTSIDE_WINDOW',
      'Pointer coordinate is outside the approved Viewer window',
      { point, window },
    )
  }

  return {
    x: window.x + point.x,
    y: window.y + point.y,
  }
}

function isContainedPath(root, candidate) {
  const relative = path.relative(root, candidate)
  return relative === '' || (!relative.startsWith(`..${path.sep}`) && relative !== '..' && !path.isAbsolute(relative))
}

function validateLiteralAbsolutePath(candidate, code) {
  if (
    typeof candidate !== 'string' ||
    candidate.length === 0 ||
    !path.isAbsolute(candidate) ||
    /[$~*?\[\]{}]/.test(candidate)
  ) {
    throw new AcceptanceError(code, 'Path must be a literal absolute path', {
      candidate,
    })
  }

  return path.normalize(candidate)
}

async function nearestExistingAncestor(candidate) {
  let current = candidate
  while (true) {
    try {
      await lstat(current)
      return current
    } catch (error) {
      if (error?.code !== 'ENOENT') throw error
      const parent = path.dirname(current)
      if (parent === current) throw error
      current = parent
    }
  }
}

async function assertNoSymlinkBetween(root, candidate, code) {
  const relative = path.relative(root, candidate)
  if (!isContainedPath(root, candidate)) {
    throw new AcceptanceError(code, 'Path escapes the approved root', {
      root,
      candidate,
    })
  }

  let current = root
  const segments = relative === '' ? [] : relative.split(path.sep)
  for (const segment of segments) {
    current = path.join(current, segment)
    try {
      const metadata = await lstat(current)
      if (metadata.isSymbolicLink()) {
        throw new AcceptanceError(code, 'Symbolic links are not accepted', {
          candidate,
          symbolicLink: current,
        })
      }
    } catch (error) {
      if (error?.code === 'ENOENT') break
      throw error
    }
  }
}

async function validateContainedPath(candidate, approvedRoot, code) {
  const normalized = validateLiteralAbsolutePath(candidate, code)
  const normalizedRoot = path.normalize(approvedRoot)

  if (!isContainedPath(normalizedRoot, normalized)) {
    throw new AcceptanceError(code, 'Path escapes the approved root', {
      root: normalizedRoot,
      candidate: normalized,
    })
  }

  const [realRoot, existingAncestor] = await Promise.all([
    realpath(normalizedRoot),
    nearestExistingAncestor(normalized),
  ])
  const realAncestor = await realpath(existingAncestor)
  if (!isContainedPath(realRoot, realAncestor)) {
    throw new AcceptanceError(code, 'Resolved path escapes the approved root', {
      root: realRoot,
      candidate: realAncestor,
    })
  }

  await assertNoSymlinkBetween(normalizedRoot, normalized, code)
  return normalized
}

export async function validateFixturePath(candidate, { repoRoot, runId }) {
  if (!/^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/.test(runId)) {
    throw new AcceptanceError('SAFETY_FIXTURE_PATH', 'Invalid fixture run ID', {
      runId,
    })
  }

  const approvedRoot = path.join(
    repoRoot,
    'target',
    'atlas-product-migration-fixture',
    'runs',
    runId,
  )
  return validateContainedPath(candidate, approvedRoot, 'SAFETY_FIXTURE_PATH')
}

export async function validateEvidencePath(
  candidate,
  { repoRoot, commit, viewport, id },
) {
  if (
    !/^[0-9a-f]{6,64}$/.test(commit) ||
    !['1024x720', '1440x900'].includes(viewport) ||
    !/^[A-Z]+-\d{2}$/.test(id)
  ) {
    throw new AcceptanceError(
      'SAFETY_EVIDENCE_PATH',
      'Invalid evidence path context',
      { commit, viewport, id },
    )
  }

  const approvedRoot = path.join(
    repoRoot,
    'target',
    'atlas-product-migration-acceptance',
    commit,
    viewport,
    id,
  )
  return validateContainedPath(candidate, approvedRoot, 'SAFETY_EVIDENCE_PATH')
}

function isRecord(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

function hasExactKeys(value, expected) {
  if (!isRecord(value)) return false
  const actual = Object.keys(value).sort()
  const wanted = [...expected].sort()
  return actual.length === wanted.length && actual.every((key, index) => key === wanted[index])
}

function hasOnlyKeys(value, allowed) {
  return isRecord(value) && Object.keys(value).every((key) => allowed.includes(key))
}

function commandError(message, details = {}) {
  return new AcceptanceError('SAFETY_COMMAND', message, details)
}

function validateTarget(target) {
  const allowed = ['role', 'name', 'identifier']
  if (!hasOnlyKeys(target, allowed) || Object.keys(target).length === 0) {
    throw commandError('Target must contain only approved selector fields')
  }
  for (const value of Object.values(target)) {
    if (typeof value !== 'string' || value.length === 0 || [...value].length > 256) {
      throw commandError('Target selector values must be bounded non-empty strings')
    }
  }
}

function validatePayload(command, payload, window) {
  if (!isRecord(payload)) throw commandError('Command payload must be an object')

  switch (command) {
    case 'inspect':
    case 'shutdown':
      if (!hasExactKeys(payload, [])) throw commandError(`${command} takes no payload fields`)
      return
    case 'query':
    case 'activate':
    case 'focus':
      if (!hasExactKeys(payload, ['target'])) throw commandError(`${command} requires a target`)
      validateTarget(payload.target)
      return
    case 'setValue':
      if (!hasExactKeys(payload, ['target', 'text'])) {
        throw commandError('setValue requires target and text')
      }
      validateTarget(payload.target)
      if (typeof payload.text !== 'string' || [...payload.text].length > 4096) {
        throw commandError('setValue text exceeds the approved limit')
      }
      return
    case 'key': {
      if (!hasExactKeys(payload, ['key', 'modifiers'])) {
        throw commandError('key requires key and modifiers')
      }
      if (!ALLOWED_KEYS.has(payload.key) || !Array.isArray(payload.modifiers)) {
        throw commandError('Unsupported key input')
      }
      const uniqueModifiers = new Set(payload.modifiers)
      if (
        uniqueModifiers.size !== payload.modifiers.length ||
        payload.modifiers.some((modifier) => !ALLOWED_MODIFIERS.has(modifier))
      ) {
        throw commandError('Unsupported or duplicate key modifier')
      }
      return
    }
    case 'pointer':
      if (!hasExactKeys(payload, ['kind', 'point'])) {
        throw commandError('pointer requires kind and point')
      }
      if (!['click', 'doubleClick', 'rightClick'].includes(payload.kind)) {
        throw commandError('Unsupported pointer action')
      }
      try {
        validateWindowPoint(payload.point, window)
      } catch (error) {
        throw commandError('Pointer coordinate is outside the approved window', {
          cause: error.code,
        })
      }
      return
    case 'drag':
      if (!hasExactKeys(payload, ['from', 'to', 'durationMs'])) {
        throw commandError('drag requires from, to and durationMs')
      }
      if (
        !Number.isInteger(payload.durationMs) ||
        payload.durationMs < 50 ||
        payload.durationMs > 5000
      ) {
        throw commandError('Drag duration is outside the approved range')
      }
      try {
        validateWindowPoint(payload.from, window)
        validateWindowPoint(payload.to, window)
      } catch (error) {
        throw commandError('Drag coordinate is outside the approved window', {
          cause: error.code,
        })
      }
      return
    case 'capture':
      if (!hasExactKeys(payload, ['path']) || typeof payload.path !== 'string') {
        throw commandError('capture requires a path')
      }
      return
    default:
      throw commandError('Unsupported command')
  }
}

export function validateCommand(request, { window } = {}) {
  const envelopeKeys = [
    'version',
    'sequence',
    'pid',
    'windowId',
    'command',
    'timeoutMs',
    'payload',
  ]
  if (!hasExactKeys(request, envelopeKeys)) {
    throw new AcceptanceError(
      'SAFETY_PROTOCOL',
      'Request envelope fields do not match protocol version 1',
    )
  }
  if (
    request.version !== PROTOCOL_VERSION ||
    !Number.isInteger(request.sequence) ||
    request.sequence <= 0 ||
    !Number.isInteger(request.pid) ||
    request.pid <= 0 ||
    !Number.isInteger(request.windowId) ||
    request.windowId <= 0 ||
    !ALLOWED_COMMANDS.has(request.command) ||
    !Number.isInteger(request.timeoutMs) ||
    request.timeoutMs < 100 ||
    request.timeoutMs > 10000
  ) {
    throw new AcceptanceError(
      'SAFETY_PROTOCOL',
      'Request envelope values violate protocol version 1',
    )
  }

  validatePayload(request.command, request.payload, window)
  return request
}

function validateResponseEnvelope(response) {
  if (!isRecord(response)) return false
  const expectedKeys = response.ok
    ? ['version', 'sequence', 'ok', 'result']
    : ['version', 'sequence', 'ok', 'error']
  return (
    hasExactKeys(response, expectedKeys) &&
    response.version === PROTOCOL_VERSION &&
    Number.isInteger(response.sequence) &&
    response.sequence > 0 &&
    typeof response.ok === 'boolean'
  )
}

export class NativeAcceptanceClient {
  constructor({
    executablePath,
    args = [],
    env = process.env,
    pid,
    window,
    defaultTimeoutMs = 3000,
  }) {
    this.executablePath = executablePath
    this.args = args
    this.env = env
    this.pid = pid
    this.window = window
    this.defaultTimeoutMs = defaultTimeoutMs
    this.sequence = 0
    this.child = undefined
    this.stderr = ''
    this.pending = new Map()
    this.exitPromise = Promise.resolve()
  }

  async start() {
    if (this.child) {
      throw new AcceptanceError(
        'PRECONDITION_HELPER_STATE',
        'Native acceptance helper is already running',
      )
    }

    const child = spawn(this.executablePath, this.args, {
      env: this.env,
      stdio: ['pipe', 'pipe', 'pipe'],
    })
    this.child = child
    child.stdout.setEncoding('utf8')
    child.stderr.setEncoding('utf8')
    child.stderr.on('data', (chunk) => {
      this.stderr = `${this.stderr}${chunk}`.slice(-32 * 1024)
    })

    const lines = readline.createInterface({ input: child.stdout })
    lines.on('line', (line) => this.handleResponseLine(line))
    child.once('error', (error) => {
      this.failPending(
        new AcceptanceError(
          'PRECONDITION_HELPER_EXIT',
          'Native acceptance helper failed to start',
          { cause: error.message, stderr: this.stderr },
        ),
      )
    })
    this.exitPromise = new Promise((resolve) => {
      child.once('close', (code, signal) => {
        lines.close()
        if (this.child === child) this.child = undefined
        this.failPending(
          new AcceptanceError(
            'PRECONDITION_HELPER_EXIT',
            'Native acceptance helper exited before responding',
            { exitCode: code, signal, stderr: this.stderr },
          ),
        )
        resolve({ code, signal })
      })
    })

    return this.request('inspect', {})
  }

  handleResponseLine(line) {
    let response
    try {
      response = JSON.parse(line)
    } catch {
      this.failProtocol('Native acceptance helper returned malformed JSON')
      return
    }

    if (!validateResponseEnvelope(response)) {
      this.failProtocol('Native acceptance helper returned an invalid envelope')
      return
    }

    const pending = this.pending.get(response.sequence)
    if (!pending) {
      this.failProtocol('Native acceptance helper returned an unexpected sequence', {
        sequence: response.sequence,
      })
      return
    }

    this.pending.delete(response.sequence)
    clearTimeout(pending.timer)
    if (response.ok) {
      pending.resolve(response.result)
      return
    }

    const error = isRecord(response.error) ? response.error : {}
    pending.reject(
      new AcceptanceError(
        typeof error.code === 'string' ? error.code : 'SAFETY_PROTOCOL',
        typeof error.message === 'string' ? error.message : 'Native helper rejected command',
        { native: error, stderr: this.stderr },
      ),
    )
  }

  failProtocol(message, details = {}) {
    this.failPending(
      new AcceptanceError('SAFETY_PROTOCOL', message, {
        ...details,
        stderr: this.stderr,
      }),
    )
    void this.terminate()
  }

  failPending(error) {
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timer)
      pending.reject(error)
    }
    this.pending.clear()
  }

  request(command, payload, { timeoutMs = this.defaultTimeoutMs } = {}) {
    const child = this.child
    if (!child || child.exitCode !== null || child.signalCode !== null) {
      return Promise.reject(
        new AcceptanceError(
          'PRECONDITION_HELPER_EXIT',
          'Native acceptance helper is not running',
          { stderr: this.stderr },
        ),
      )
    }

    const request = validateCommand(
      {
        version: PROTOCOL_VERSION,
        sequence: ++this.sequence,
        pid: this.pid,
        windowId: this.window.windowId,
        command,
        timeoutMs,
        payload,
      },
      { window: this.window },
    )

    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(request.sequence)
        reject(
          new AcceptanceError(
            'PRECONDITION_HELPER_TIMEOUT',
            'Native acceptance helper response timed out',
            { sequence: request.sequence, command, stderr: this.stderr },
          ),
        )
        void this.terminate()
      }, timeoutMs)
      this.pending.set(request.sequence, { resolve, reject, timer })
      child.stdin.write(`${JSON.stringify(request)}\n`, (error) => {
        if (!error) return
        const pending = this.pending.get(request.sequence)
        if (!pending) return
        this.pending.delete(request.sequence)
        clearTimeout(pending.timer)
        reject(
          new AcceptanceError(
            'PRECONDITION_HELPER_EXIT',
            'Failed to write to native acceptance helper',
            { cause: error.message, stderr: this.stderr },
          ),
        )
      })
    })
  }

  async close() {
    if (!this.child) return
    try {
      await this.request('shutdown', {})
      this.child?.stdin.end()
      await this.exitPromise
    } catch (error) {
      await this.terminate()
      throw error
    }
  }

  async terminate() {
    const child = this.child
    if (!child) return
    this.child = undefined
    this.failPending(
      new AcceptanceError(
        'PRECONDITION_HELPER_EXIT',
        'Native acceptance helper was terminated',
        { stderr: this.stderr },
      ),
    )
    if (child.exitCode === null && child.signalCode === null) child.kill('SIGKILL')
    child.stdin.destroy()
    await this.exitPromise
  }
}

export async function buildNativeHelper({ repoRoot, sourcePath }) {
  const source = await readFile(sourcePath)
  const sourceHash = createHash('sha256').update(source).digest('hex')
  const buildDirectory = path.join(
    repoRoot,
    'target',
    'native-acceptance-tools',
    sourceHash,
  )
  const executablePath = path.join(
    buildDirectory,
    'viewer-native-acceptance-helper',
  )

  try {
    await access(executablePath)
    return { sourceHash, executablePath }
  } catch (error) {
    if (error?.code !== 'ENOENT') throw error
  }

  await mkdir(buildDirectory, { recursive: true })
  const temporaryPath = `${executablePath}.${process.pid}.${randomUUID()}`
  try {
    await execFileAsync('xcrun', [
      'swiftc',
      '-warnings-as-errors',
      sourcePath,
      '-o',
      temporaryPath,
    ])
    await rename(temporaryPath, executablePath)
  } catch (error) {
    await rm(temporaryPath, { force: true })
    throw error
  }

  return { sourceHash, executablePath }
}
