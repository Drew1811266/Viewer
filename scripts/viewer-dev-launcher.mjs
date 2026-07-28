import path from 'node:path'
import { fileURLToPath } from 'node:url'

/**
 * @typedef {{
 *   repoRoot: string,
 *   stateDir: string,
 *   statePath: string,
 *   logPath: string,
 *   executablePath: string,
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
 * @param {string} command
 * @param {string} repoRoot
 * @returns {boolean}
 */
export function isViewerExecutable(command, repoRoot) {
  const executable = commandExecutable(command)
  const repositoryViewer =
    executable.startsWith(`${repoRoot}${path.sep}`) &&
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
  return (
    command.includes(`${repoRoot}${path.sep}`) &&
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
