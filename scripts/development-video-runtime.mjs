import { execFile } from 'node:child_process'
import { cp, lstat, mkdir, mkdtemp, readFile, rename, rm } from 'node:fs/promises'
import path from 'node:path'
import { promisify } from 'node:util'

const execFileAsync = promisify(execFile)

async function readRuntimeIdentity(runtimePath) {
  const [inventory, lock] = await Promise.all([
    readFile(path.join(runtimePath, 'runtime.inventory.sha256'), 'utf8'),
    readFile(path.join(runtimePath, 'runtime.lock.json'), 'utf8'),
  ])

  return `${inventory}\0${lock}`
}

async function safeDirectoryExists(directory) {
  try {
    const metadata = await lstat(directory)
    return metadata.isDirectory() && !metadata.isSymbolicLink()
  } catch (error) {
    if (error?.code === 'ENOENT') return false
    throw error
  }
}

async function verifyRuntime(verifierPath, runtimePath, env) {
  try {
    await execFileAsync('/bin/bash', [verifierPath], {
      env: {
        ...env,
        VIEWER_VIDEO_STAGE_DIR: runtimePath,
      },
      maxBuffer: 4 * 1024 * 1024,
    })
  } catch (error) {
    const detail = error?.stderr?.trim() || error?.message || 'unknown verification error'
    throw new Error(
      `Viewer development video runtime verification failed at ${runtimePath}: ${detail}`,
      { cause: error },
    )
  }
}

/**
 * Materialize the reviewed video runtime at the direct Tauri development
 * resource path. Production bundle lookup remains strict and unaware of build
 * tree layouts.
 *
 * @param {{
 *   sourcePath: string,
 *   destinationPath: string,
 *   verifierPath: string,
 *   env?: NodeJS.ProcessEnv,
 * }} options
 * @returns {Promise<'installed' | 'reused'>}
 */
export async function prepareDevelopmentVideoRuntime({
  sourcePath,
  destinationPath,
  verifierPath,
  env = process.env,
}) {
  if (!(await safeDirectoryExists(sourcePath))) {
    throw new Error(`Viewer development video runtime source is unavailable: ${sourcePath}`)
  }

  await verifyRuntime(verifierPath, sourcePath, env)
  const sourceIdentity = await readRuntimeIdentity(sourcePath)

  if (await safeDirectoryExists(destinationPath)) {
    try {
      await verifyRuntime(verifierPath, destinationPath, env)
      if ((await readRuntimeIdentity(destinationPath)) === sourceIdentity) {
        return 'reused'
      }
    } catch {
      // A stale or incomplete target is replaced from the verified source below.
    }
  }

  const destinationParent = path.dirname(destinationPath)
  await mkdir(destinationParent, { recursive: true })
  const workspace = await mkdtemp(
    path.join(destinationParent, '.viewer-video-runtime-prepare-'),
  )
  const candidatePath = path.join(workspace, 'ViewerVideoRuntime')
  const backupPath = path.join(workspace, 'previous-runtime')
  let destinationBackedUp = false

  try {
    await cp(sourcePath, candidatePath, {
      recursive: true,
      force: false,
      errorOnExist: true,
      preserveTimestamps: true,
    })
    await verifyRuntime(verifierPath, candidatePath, env)

    try {
      await rename(destinationPath, backupPath)
      destinationBackedUp = true
    } catch (error) {
      if (error?.code !== 'ENOENT') throw error
    }

    try {
      await rename(candidatePath, destinationPath)
    } catch (error) {
      if (destinationBackedUp) {
        await rename(backupPath, destinationPath)
        destinationBackedUp = false
      }
      throw error
    }

    return 'installed'
  } finally {
    await rm(workspace, { recursive: true, force: true })
  }
}
