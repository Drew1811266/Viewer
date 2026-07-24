#!/usr/bin/env node

import { execFileSync, spawnSync } from 'node:child_process'
import { pathToFileURL } from 'node:url'

const entries = (buffer) =>
  buffer
    .toString('utf8')
    .split('\0')
    .filter(Boolean)
    .sort()

export function comparePorcelain(before, after) {
  const baseline = new Set(entries(before))
  const current = new Set(entries(after))
  return [
    ...[...baseline].filter((entry) => !current.has(entry)).map((entry) => `removed: ${entry}`),
    ...[...current].filter((entry) => !baseline.has(entry)).map((entry) => `added: ${entry}`),
  ].sort()
}

function porcelain() {
  return execFileSync('git', ['status', '--porcelain=v1', '-z'], { encoding: 'buffer' })
}

export function main() {
  const before = porcelain()
  const result = spawnSync('pnpm', ['verify'], { stdio: 'inherit' })
  const changes = comparePorcelain(before, porcelain())
  if (changes.length > 0) {
    process.stderr.write(`verification changed worktree entries:\n${changes.join('\n')}\n`)
    return 1
  }
  return result.status ?? 1
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) process.exitCode = main()
