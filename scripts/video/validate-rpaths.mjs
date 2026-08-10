import { readFile } from 'node:fs/promises'
import { pathToFileURL } from 'node:url'

const allowedRpaths = new Set(['/usr/lib/swift'])

export function validateRpaths(output) {
  const lines = output.split('\n')
  const rpaths = []
  for (let index = 0; index < lines.length; index += 1) {
    if (lines[index].trim() !== 'cmd LC_RPATH') continue
    for (let offset = 1; offset <= 3 && index + offset < lines.length; offset += 1) {
      const match = lines[index + offset].trim().match(/^path (.+) \(offset \d+\)$/)
      if (match) {
        rpaths.push(match[1])
        break
      }
    }
  }

  for (const rpath of rpaths) {
    if (!allowedRpaths.has(rpath) && !rpath.startsWith('@')) {
      throw new Error(`prohibited runtime search path: ${rpath}`)
    }
  }
}

async function main([path]) {
  if (!path) {
    throw new Error('usage: validate-rpaths.mjs <otool-load-commands.txt>')
  }
  validateRpaths(await readFile(path, 'utf8'))
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  main(process.argv.slice(2)).catch((error) => {
    process.stderr.write(`${error.message}\n`)
    process.exitCode = 1
  })
}
