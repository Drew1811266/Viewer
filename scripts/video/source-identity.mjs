import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFileSync, statSync } from 'node:fs'
import path from 'node:path'

export function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex')
}

export function collectSourceIdentity(sourceRoot) {
  const commit = git(sourceRoot, ['rev-parse', 'HEAD']).toString().trim()
  const trackedDiffSha256 = sha256(
    git(sourceRoot, ['diff', '--binary', '--no-ext-diff', 'HEAD', '--']),
  )
  const untracked = git(sourceRoot, ['ls-files', '--others', '--exclude-standard', '-z'])
    .toString()
    .split('\0')
    .filter(Boolean)
    .sort()
    .map((relativePath) => {
      const filePath = path.join(sourceRoot, relativePath)
      const metadata = statSync(filePath)
      if (!metadata.isFile()) {
        throw new Error(`untracked source identity entry is not a regular file: ${relativePath}`)
      }
      return {
        path: relativePath,
        mode: metadata.mode & 0o777,
        sha256: sha256(readFileSync(filePath)),
      }
    })
  const manifest = { commit, trackedDiffSha256, untracked }
  return { ...manifest, manifestSha256: sha256(JSON.stringify(manifest)) }
}

function git(sourceRoot, argumentsList) {
  const result = spawnSync('git', argumentsList, {
    cwd: sourceRoot,
    encoding: null,
    maxBuffer: 64 * 1024 * 1024,
  })
  if (result.error) throw result.error
  if (result.status !== 0) {
    throw new Error(`git ${argumentsList.join(' ')} failed while collecting source identity`)
  }
  return result.stdout
}
