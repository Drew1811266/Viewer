#!/usr/bin/env node

import { execFile } from 'node:child_process'
import path from 'node:path'
import { promisify } from 'node:util'
import { pathToFileURL } from 'node:url'

import {
  buildLauncherPaths,
  createSystemRuntime,
  restartDevelopmentViewer,
} from './viewer-dev-launcher.mjs'

const execFileAsync = promisify(execFile)

/**
 * @param {{
 *   moduleUrl?: string,
 *   runtimeFactory?: typeof createSystemRuntime,
 *   restart?: typeof restartDevelopmentViewer,
 *   execGit?: (args: string[], options: {cwd: string}) => Promise<{stdout: string}>,
 *   log?: (line: string) => void,
 * }} [options]
 */
export async function runCli({
  moduleUrl = import.meta.url,
  runtimeFactory = createSystemRuntime,
  restart = restartDevelopmentViewer,
  execGit = (args, options) => execFileAsync('git', args, options),
  log = console.log,
} = {}) {
  const paths = buildLauncherPaths(moduleUrl)
  const runtime = runtimeFactory(paths)
  const result = await restart({ paths, runtime })
  const { stdout: branch } = await execGit(
    ['rev-parse', '--abbrev-ref', 'HEAD'],
    { cwd: paths.repoRoot },
  )
  const { stdout: commit } = await execGit(
    ['rev-parse', '--short', 'HEAD'],
    { cwd: paths.repoRoot },
  )
  const { stdout: dirty } = await execGit(
    ['status', '--porcelain'],
    { cwd: paths.repoRoot },
  )

  log(`Viewer development version is running: ${result.executablePath}`)
  log(`Source: ${branch.trim()} @ ${commit.trim()}${dirty ? ' (dirty)' : ''}`)
  log(`Log: ${result.logPath}`)

  return result
}

const entryUrl = process.argv[1]
  ? pathToFileURL(path.resolve(process.argv[1])).href
  : undefined

if (entryUrl === import.meta.url) {
  runCli().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error))
    process.exitCode = 1
  })
}
