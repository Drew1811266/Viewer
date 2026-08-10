import { readFile } from 'node:fs/promises'
import { pathToFileURL } from 'node:url'

const fail = (message) => {
  throw new Error(message)
}

export async function loadRuntimeLock(path) {
  const lock = JSON.parse(await readFile(path, 'utf8'))
  if (lock.schemaVersion !== 1) fail('unsupported runtime lock schema')
  if (lock.target !== 'universal-apple-darwin') fail('unsupported runtime lock target')
  if (lock.mpv?.mesonOptions?.gpl !== 'false') fail('mpv must be built with gpl=false')
  for (const option of ['--disable-gpl', '--disable-nonfree', '--disable-network']) {
    if (!lock.ffmpeg?.configureOptions?.includes(option)) {
      fail(`ffmpeg is missing ${option}`)
    }
  }
  if (!Array.isArray(lock.components) || lock.components.length === 0) {
    fail('runtime lock has no components')
  }
  const names = new Set()
  for (const component of lock.components) {
    if (!component.name || names.has(component.name)) fail('duplicate or empty component name')
    names.add(component.name)
    if (!component.version) fail(`${component.name} has no version`)
    if (!/^https:\/\//.test(component.sourceUrl)) fail(`${component.name} has an invalid source URL`)
    if (!/^[a-f0-9]{64}$/.test(component.sha256)) {
      fail(`${component.name} has an invalid SHA-256`)
    }
    if (!component.license || /(^|-)GPL(?:-|$)/.test(component.license)) {
      fail(`${component.name} has a prohibited license selection`)
    }
    for (const value of [component.name, component.version, component.sourceUrl, component.sha256]) {
      if (/[\t\r\n]/.test(value)) fail(`${component.name} has an unsafe field`)
    }
  }
  return lock
}

async function main([command, path]) {
  if (!['validate', 'list'].includes(command) || !path) {
    fail('usage: runtime-lock.mjs <validate|list> <runtime.lock.json>')
  }
  const lock = await loadRuntimeLock(path)
  if (command === 'list') {
    for (const component of lock.components) {
      process.stdout.write(
        `${component.name}\t${component.version}\t${component.sourceUrl}\t${component.sha256}\n`,
      )
    }
  }
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  main(process.argv.slice(2)).catch((error) => {
    process.stderr.write(`${error.message}\n`)
    process.exitCode = 1
  })
}
