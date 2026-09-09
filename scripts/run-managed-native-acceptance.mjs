#!/usr/bin/env node

import { pathToFileURL } from 'node:url'
import {
  buildLauncherPaths,
  createSystemRuntime,
  restartDevelopmentViewer,
} from './viewer-dev-launcher.mjs'
import { runNativeAcceptanceCli } from './viewer-native-acceptance.mjs'

/**
 * @param {string[]} argv
 * @returns {string | undefined}
 */
export function buildAcceptanceWindowConfig(argv) {
  const index = argv.findIndex((argument) => argument === '--viewport')
  const viewport = index === -1 ? undefined : argv[index + 1]
  const match = viewport?.match(/^(1024|1440)x(720|900)$/)
  if (!match) return undefined
  const width = Number(match[1])
  const height = Number(match[2])
  if ((width === 1024 && height !== 720) || (width === 1440 && height !== 900)) {
    return undefined
  }
  return JSON.stringify({ app: { windows: [{ width, height }] } })
}

/**
 * Start the exact current-worktree development process, run native acceptance
 * against the PID returned by the launcher, and tear down that same session.
 * This is deliberately separate from the product and never opens a bundle by
 * name or through LaunchServices.
 *
 * @param {string[]} [argv]
 * @param {{
 *   moduleUrl?: string,
 *   runtimeFactory?: typeof createSystemRuntime,
 *   restart?: typeof restartDevelopmentViewer,
 *   runAcceptance?: typeof runNativeAcceptanceCli,
 * }} [options]
 */
export async function runManagedNativeAcceptance(
  argv = process.argv.slice(2),
  {
    moduleUrl = import.meta.url,
    runtimeFactory = createSystemRuntime,
    restart = restartDevelopmentViewer,
    runAcceptance = runNativeAcceptanceCli,
  } = {},
) {
  const paths = buildLauncherPaths(moduleUrl)
  const env = { ...process.env }
  // Native acceptance requires one of the two exact window sizes. Supplying
  // the override as inline JSON keeps the managed command self-contained and
  // avoids creating a second config file that could become stale.
  if (!env.VIEWER_TAURI_CONFIG) {
    const config = buildAcceptanceWindowConfig(argv)
    if (config) env.VIEWER_TAURI_CONFIG = config
  }
  const runtime = runtimeFactory(paths, { env })
  const launch = await restart({ paths, runtime })
  try {
    return await runAcceptance(argv, { repoRoot: paths.repoRoot })
  } finally {
    // The launcher process group is the only process identity this command is
    // allowed to stop.  Do not rediscover by application name or executable
    // basename during teardown.
    await runtime
      .stop({ kind: 'group', id: launch.pid, viewerPid: launch.viewerPid })
      .catch(() => {})
    await runtime.clearSession().catch(() => {})
  }
}

const invokedPath = process.argv[1] ? pathToFileURL(process.argv[1]).href : ''
if (invokedPath === import.meta.url) {
  try {
    const result = await runManagedNativeAcceptance()
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
    process.exitCode = result?.exitCode ?? 0
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`)
    process.exitCode = 1
  }
}
