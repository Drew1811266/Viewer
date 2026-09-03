import { execFile, spawn as spawnChild } from 'node:child_process'
import { constants as fsConstants } from 'node:fs'
import { access, mkdir, open, readFile, rename, rm, stat, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { promisify } from 'node:util'
import { fileURLToPath } from 'node:url'

import { prepareDevelopmentVideoRuntime } from './development-video-runtime.mjs'

const execFileAsync = promisify(execFile)

/**
 * @typedef {{
 *   repoRoot: string,
 *   stateDir: string,
 *   statePath: string,
 *   logPath: string,
 *   executablePath: string,
 *   legacyWrapperPath: string,
 *   videoRuntimeSourcePath: string,
 *   videoRuntimeDestinationPath: string,
 *   videoRuntimeVerifierPath: string,
 * }} LauncherPaths
 *
 * @typedef {{
 *   pid: number,
 *   ppid: number,
 *   pgid: number,
 *   command: string,
 * }} ProcessInfo
 *
 * @typedef {{
 *   kind: 'group' | 'process',
 *   id: number,
 *   viewerPid: number,
 * }} StopTarget
 */

/**
 * @param {string} moduleUrl
 * @returns {LauncherPaths}
 */
export function buildLauncherPaths(moduleUrl) {
  const scriptPath = fileURLToPath(moduleUrl)
  const repoRoot = path.dirname(path.dirname(scriptPath))
  const stateDir = path.join(repoRoot, 'target', 'dev-launcher')

  return {
    repoRoot,
    stateDir,
    statePath: path.join(stateDir, 'session.json'),
    logPath: path.join(stateDir, 'tauri-dev.log'),
    executablePath: path.join(repoRoot, 'target', 'debug', 'viewer-desktop'),
    legacyWrapperPath: path.join(stateDir, 'current-dev-wrapper'),
    videoRuntimeSourcePath: path.join(
      repoRoot,
      'target',
      'viewer-video-runtime',
      'universal-apple-darwin',
      'ViewerVideoRuntime',
    ),
    videoRuntimeDestinationPath: path.join(
      repoRoot,
      'target',
      'debug',
      'ViewerVideoRuntime',
    ),
    videoRuntimeVerifierPath: path.join(
      repoRoot,
      'scripts',
      'video',
      'verify-runtime.sh',
    ),
  }
}

/**
 * @param {string} output
 * @returns {ProcessInfo[]}
 */
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

/**
 * @param {string} command
 * @returns {string}
 */
function commandExecutable(command) {
  return command.trim().split(/\s+/, 1)[0]
}

/**
 * @param {string} repoRoot
 * @returns {string}
 */
function viewerProcessScopeRoot(repoRoot) {
  const worktreeMarker = `${path.sep}.worktrees${path.sep}`
  const markerIndex = repoRoot.indexOf(worktreeMarker)

  return markerIndex === -1 ? repoRoot : repoRoot.slice(0, markerIndex)
}

/**
 * @param {string} command
 * @param {string} repoRoot
 * @returns {boolean}
 */
export function isViewerExecutable(command, repoRoot) {
  const executable = commandExecutable(command)
  const scopeRoot = viewerProcessScopeRoot(repoRoot)
  const repositoryViewer =
    executable.startsWith(`${scopeRoot}${path.sep}`) &&
    executable.endsWith(`${path.sep}viewer-desktop`)
  const packagedViewer = executable.endsWith(
    `${path.sep}Viewer.app${path.sep}Contents${path.sep}MacOS${path.sep}viewer-desktop`,
  )

  return repositoryViewer || packagedViewer
}

/**
 * @param {string} command
 * @param {string} repoRoot
 * @returns {boolean}
 */
export function isTauriDevProcess(command, repoRoot) {
  const scopeRoot = viewerProcessScopeRoot(repoRoot)

  return (
    command.includes(`${scopeRoot}${path.sep}`) &&
    command.includes(`${path.sep}node_modules${path.sep}`) &&
    /(?:tauri\.js|tauri)\s+["']?dev["']?(?:\s|$)/.test(command)
  )
}

/**
 * @param {ProcessInfo[]} processes
 * @param {string} repoRoot
 * @param {number} [currentPid]
 * @returns {StopTarget[]}
 */
export function selectStopTargets(processes, repoRoot, currentPid = process.pid) {
  const byPid = new Map(processes.map((item) => [item.pid, item]))
  const currentGroup = byPid.get(currentPid)?.pgid
  /** @type {StopTarget[]} */
  const targets = []
  const seen = new Set()

  for (const viewer of processes.filter((item) =>
    isViewerExecutable(item.command, repoRoot),
  )) {
    let ancestor = viewer
    let hasTauriAncestor = false

    while (ancestor) {
      if (isTauriDevProcess(ancestor.command, repoRoot)) {
        hasTauriAncestor = true
        break
      }
      ancestor = byPid.get(ancestor.ppid)
    }

    const useGroup = hasTauriAncestor && viewer.pgid > 1 && viewer.pgid !== currentGroup
    const target = useGroup
      ? { kind: 'group', id: viewer.pgid, viewerPid: viewer.pid }
      : { kind: 'process', id: viewer.pid, viewerPid: viewer.pid }
    const key = `${target.kind}:${target.id}`

    if (!seen.has(key)) {
      seen.add(key)
      targets.push(target)
    }
  }

  return targets
}

/**
 * @param {ProcessInfo[]} processes
 * @param {string} executablePath
 * @returns {boolean}
 */
export function isExactDevelopmentViewerRunning(processes, executablePath) {
  return processes.some((item) => commandExecutable(item.command) === executablePath)
}

/**
 * @param {ProcessInfo[]} processes
 * @param {string} repoRoot
 * @param {string} executablePath
 * @returns {{exact: ProcessInfo[], eligible: ProcessInfo[]}}
 */
export function inspectDevelopmentViewers(processes, repoRoot, executablePath) {
  return {
    exact: processes.filter(
      (item) => commandExecutable(item.command) === executablePath,
    ),
    eligible: processes.filter((item) => isViewerExecutable(item.command, repoRoot)),
  }
}

/**
 * @typedef {{
 *   version: 1,
 *   pid: number,
 *   pgid: number,
 *   repoRoot: string,
 *   startedAt: string,
 * }} SessionState
 *
 * @typedef {{
 *   removeLegacyWrapper(): Promise<void>,
 *   listProcesses(): Promise<ProcessInfo[]>,
 *   readSession(): Promise<SessionState | undefined>,
 *   writeSession(state: SessionState): Promise<void>,
 *   clearSession(): Promise<void>,
 *   stop(target: StopTarget): Promise<void>,
 *   spawn(): Promise<{pid: number, pgid: number}>,
 *   isAlive(pid: number): boolean,
 *   sleep(ms: number): Promise<void>,
 *   tailLog(lines: number): Promise<string>,
 * }} LauncherRuntime
 */

/**
 * @param {number} pid
 * @returns {boolean}
 */
function isProcessAlive(pid) {
  try {
    process.kill(pid, 0)
    return true
  } catch (error) {
    if (error?.code === 'ESRCH') return false
    if (error?.code === 'EPERM') return true
    throw error
  }
}

/**
 * @param {StopTarget} target
 * @returns {boolean}
 */
function isTargetAlive(target) {
  const pid = target.kind === 'group' ? -target.id : target.id
  return isProcessAlive(pid)
}

/**
 * @param {number} ms
 * @returns {Promise<void>}
 */
function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

/**
 * @param {LauncherPaths} paths
 * @param {{env?: NodeJS.ProcessEnv}} [options]
 * @returns {LauncherRuntime}
 */
export function createSystemRuntime(paths, { env = process.env } = {}) {
  return {
    async removeLegacyWrapper() {
      await rm(paths.legacyWrapperPath, { recursive: true, force: true })
    },

    async listProcesses() {
      const { stdout } = await execFileAsync(
        'ps',
        ['-axo', 'pid=,ppid=,pgid=,command='],
        { maxBuffer: 4 * 1024 * 1024 },
      )
      return parseProcessTable(stdout)
    },

    async readSession() {
      try {
        const value = JSON.parse(await readFile(paths.statePath, 'utf8'))
        return value && typeof value === 'object' ? value : undefined
      } catch (error) {
        if (error?.code === 'ENOENT' || error instanceof SyntaxError) return undefined
        throw error
      }
    },

    async writeSession(state) {
      await mkdir(paths.stateDir, { recursive: true })
      const temporaryPath = `${paths.statePath}.${process.pid}.tmp`
      await writeFile(temporaryPath, `${JSON.stringify(state, null, 2)}\n`)
      await rename(temporaryPath, paths.statePath)
    },

    async clearSession() {
      await rm(paths.statePath, { force: true })
    },

    async stop(target) {
      const pid = target.kind === 'group' ? -target.id : target.id
      try {
        process.kill(pid, 'SIGTERM')
      } catch (error) {
        if (error?.code === 'ESRCH') return
        throw error
      }

      const deadline = Date.now() + 5_000
      while (Date.now() <= deadline) {
        if (!isTargetAlive(target)) return
        await delay(50)
      }

      throw new Error(
        `Viewer ${target.kind} ${target.id} did not stop after SIGTERM`,
      )
    },

    async spawn() {
      const packagePath = path.join(paths.repoRoot, 'package.json')
      const tauriPath = path.join(paths.repoRoot, 'src-tauri')
      try {
        await access(packagePath, fsConstants.F_OK)
        const tauriMetadata = await stat(tauriPath)
        if (!tauriMetadata.isDirectory()) {
          throw new Error(`${tauriPath} is not a directory`)
        }
      } catch (error) {
        throw new Error(`Viewer repository is incomplete: ${error.message}`, {
          cause: error,
        })
      }

      await prepareDevelopmentVideoRuntime({
        sourcePath: paths.videoRuntimeSourcePath,
        destinationPath: paths.videoRuntimeDestinationPath,
        verifierPath: paths.videoRuntimeVerifierPath,
        env,
      })

      await mkdir(paths.stateDir, { recursive: true })
      const logFile = await open(paths.logPath, 'w')
      const args = ['tauri', 'dev']
      const tauriConfig = env.VIEWER_TAURI_CONFIG?.trim()
      if (tauriConfig) args.push('--config', tauriConfig)

      return new Promise((resolve, reject) => {
        const child = spawnChild('pnpm', args, {
          cwd: paths.repoRoot,
          detached: true,
          env,
          stdio: ['ignore', logFile.fd, logFile.fd],
        })

        child.once('error', async (error) => {
          await logFile.close()
          reject(new Error(`Unable to start pnpm tauri dev: ${error.message}`, {
            cause: error,
          }))
        })
        child.once('spawn', async () => {
          await logFile.close()
          child.unref()
          resolve({ pid: child.pid, pgid: child.pid })
        })
      })
    },

    isAlive(pid) {
      return isProcessAlive(pid)
    },

    sleep(ms) {
      return delay(ms)
    },

    async tailLog(lines) {
      try {
        const contents = await readFile(paths.logPath, 'utf8')
        return contents.split(/\r?\n/).slice(-lines).join('\n')
      } catch (error) {
        if (error?.code === 'ENOENT') return ''
        throw error
      }
    },
  }
}

/**
 * @param {SessionState | undefined} session
 * @param {ProcessInfo | undefined} liveProcess
 * @param {string} repoRoot
 * @returns {boolean}
 */
export function sessionMatchesProcess(session, liveProcess, repoRoot) {
  return (
    session?.version === 1 &&
    session.repoRoot === repoRoot &&
    liveProcess?.pid === session.pid &&
    liveProcess.pgid === session.pgid &&
    /(?:pnpm\s+tauri\s+["']?dev["']?|tauri\.js\s+["']?dev["']?)(?:\s|$)/.test(
      liveProcess.command,
    )
  )
}

/**
 * @param {{
 *   paths: LauncherPaths,
 *   runtime: LauncherRuntime,
 *   timeoutMs?: number,
 *   pollMs?: number,
 * }} options
 * @returns {Promise<{
 *   pid: number,
 *   viewerPid: number,
 *   executablePath: string,
 *   logPath: string,
 * }>}
 */
export async function restartDevelopmentViewer({
  paths,
  runtime,
  timeoutMs = 120_000,
  pollMs = 250,
}) {
  await runtime.removeLegacyWrapper()
  const initial = await runtime.listProcesses()
  const stored = await runtime.readSession()
  const storedProcess = stored
    ? initial.find((item) => item.pid === stored.pid)
    : undefined

  if (stored && sessionMatchesProcess(stored, storedProcess, paths.repoRoot)) {
    await runtime.stop({ kind: 'group', id: stored.pgid, viewerPid: stored.pid })
  } else if (stored) {
    await runtime.clearSession()
  }

  for (const target of selectStopTargets(initial, paths.repoRoot)) {
    const alreadyStoppedStoredGroup =
      stored?.pgid === target.id && target.kind === 'group'
    if (!alreadyStoppedStoredGroup) {
      await runtime.stop(target)
    }
  }

  const remaining = await runtime.listProcesses()
  if (remaining.some((item) => isViewerExecutable(item.command, paths.repoRoot))) {
    throw new Error('Existing Viewer process did not stop')
  }

  const child = await runtime.spawn()
  await runtime.writeSession({
    version: 1,
    pid: child.pid,
    pgid: child.pgid,
    repoRoot: paths.repoRoot,
    startedAt: new Date().toISOString(),
  })

  const deadline = Date.now() + timeoutMs
  while (Date.now() <= deadline) {
    const processes = await runtime.listProcesses()
    const observation = inspectDevelopmentViewers(
      processes,
      paths.repoRoot,
      paths.executablePath,
    )

    if (observation.exact.length === 1 && observation.eligible.length === 1) {
      return {
        pid: child.pid,
        viewerPid: observation.exact[0].pid,
        executablePath: paths.executablePath,
        logPath: paths.logPath,
      }
    }

    const hasDuplicate =
      observation.exact.length > 1 ||
      observation.eligible.length > 1 ||
      (observation.exact.length === 0 && observation.eligible.length > 0)

    if (hasDuplicate) {
      await runtime.stop({ kind: 'group', id: child.pgid, viewerPid: child.pid })
      await runtime.clearSession()
      throw new Error(
        `Viewer development startup violated the single-instance invariant: ` +
          `exact=${observation.exact.length}, eligible=${observation.eligible.length}`,
      )
    }
    if (!runtime.isAlive(child.pid)) {
      const tail = await runtime.tailLog(40)
      await runtime.clearSession()
      throw new Error(`Viewer development launcher exited early.\n${tail}`)
    }
    await runtime.sleep(pollMs)
  }

  await runtime.stop({ kind: 'group', id: child.pgid, viewerPid: child.pid })
  await runtime.clearSession()
  const tail = await runtime.tailLog(40)
  throw new Error(`Viewer development startup timed out.\n${tail}`)
}
