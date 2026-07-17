#!/usr/bin/env node

import { spawnSync } from 'node:child_process'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const reviewedLicenseExpressions = new Set([
  'MIT',
  'MIT-0',
  'Apache-2.0',
  'Apache-2.0 OR MIT',
  'BSD-2-Clause',
  'BSD-3-Clause',
  'ISC',
  'CC0-1.0',
  'MPL-2.0',
  'BlueOak-1.0.0',
])

export function validateLicenseInventory(inventory) {
  if (!inventory || Array.isArray(inventory) || typeof inventory !== 'object') {
    throw new TypeError('pnpm license inventory must be an object')
  }

  let packageCount = 0
  for (const [expression, packages] of Object.entries(inventory)) {
    if (!Array.isArray(packages) || packages.length === 0) {
      throw new Error(`npm license expression ${expression} has no packages`)
    }
    if (!reviewedLicenseExpressions.has(expression)) {
      const identities = packages
        .map(({ name, versions }) => `${name}@${Array.isArray(versions) ? versions.join(',') : '?'}`)
        .join(', ')
      throw new Error(`unreviewed npm license expression ${expression}: ${identities}`)
    }
    packageCount += packages.length
  }

  if (packageCount === 0) {
    throw new Error('pnpm license inventory contains no packages')
  }
  return packageCount
}

function main() {
  const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
  const result = spawnSync('pnpm', ['licenses', 'list', '--json'], {
    cwd: root,
    encoding: 'utf8',
  })
  if (result.error) throw result.error
  if (result.status !== 0) {
    throw new Error(`pnpm license inventory failed: ${result.stderr.trim()}`)
  }

  const inventory = JSON.parse(result.stdout)
  const packageCount = validateLicenseInventory(inventory)
  console.log(
    `npm license policy passed: ${packageCount} packages across ${Object.keys(inventory).length} reviewed expressions`,
  )
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main()
}
