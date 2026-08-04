import path from 'node:path'
import { lstat, realpath } from 'node:fs/promises'

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
